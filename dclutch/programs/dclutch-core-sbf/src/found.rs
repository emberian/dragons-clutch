//! Exact finalized-record Found transition and prepaid Market creation.

use alloc::boxed::Box;
use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_market_core_codec::{
    Action, Admission, CoreState, FoundingAccounts, FoundingFrame, FoundingQuote,
    MarketCoreStateSeedsV2, MarketIdentity, Product, ProjectFoundReceiptV2, Realm, Request, Role,
    STATE_BYTES, VacantAccount, found,
};
use dclutch_product_runtime_v2_svm_reader::{
    AuthenticatedProductRuntimeV2, FinalizedRecordFrameV2, ProductRuntimeFrameV2,
    authenticate_product_basis_v3,
};
use dclutch_realm_contract::{REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_rent_contract::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleRentCreditV2,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
    MANIPULATION_FLOOR_V1_BYTES, ManipulationFloorV1, SOURCE_CAPACITY_PROFILE_BYTES,
    SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_MATERIAL_V3_BYTES, SOURCE_SPEC_BYTES, SOURCE_SPEC_SCHEMA_ID_V1, SourceCapacityProfileV1,
    SourceMaterialV3, SourcePrincipalPolicyV1, SourceSpecV1,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    program::{invoke, invoke_signed, set_return_data},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    CoreSbfError,
    frame::{FoundAccounts, ProjectedFoundAccountsV2},
    infrastructure::{
        authenticate_found, authenticate_immutable_core_release, authenticate_projected_found,
        authenticate_projected_immutable_core_release,
    },
    product_runtime_v2::{authenticate_product_runtime_v2, project_core_product_v2},
    records::authenticate_content_addressed_record,
    release::{authenticate_role, identity},
};

struct References {
    realm_id: [u8; 32],
    collateral_mint: [u8; 32],
    token_program: [u8; 32],
    collateral_release: [u8; 32],
    product_record_id: [u8; 32],
    product_id: [u8; 32],
    product: Product,
    resolution_policy_id: [u8; 32],
    manifest_id: [u8; 32],
    release_set_id: [u8; 32],
    principal_cap_sets: u64,
}

struct CreationPlan {
    state_seeds: MarketCoreStateSeedsV2,
    bump: u8,
    state: CoreState,
    rent_top_up: u64,
}

#[derive(Clone, Copy)]
struct FoundCommonAccounts<'accounts, 'info> {
    payer: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    rent_credit: &'accounts AccountInfo<'info>,
    rent_program: &'accounts AccountInfo<'info>,
    registry_program: &'accounts AccountInfo<'info>,
    system: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> FoundCommonAccounts<'accounts, 'info> {
    const fn ordinary(frame: &'accounts FoundAccounts<'accounts, 'info>) -> Self {
        Self {
            payer: frame.payer,
            market: frame.market,
            rent_credit: frame.rent_credit,
            rent_program: frame.rent_program,
            registry_program: frame.registry_program,
            system: frame.system,
        }
    }

    const fn projected(frame: &'accounts ProjectedFoundAccountsV2<'accounts, 'info>) -> Self {
        Self {
            payer: frame.payer,
            market: frame.market,
            rent_credit: frame.rent_credit,
            rent_program: frame.rent_program,
            registry_program: frame.registry_program,
            system: frame.system,
        }
    }
}

/// Facts admitted earlier by ordinary ProjectFound and persisted by Custody V2.
#[derive(Clone, Copy)]
pub(crate) struct ProjectedFoundAuthorityV2 {
    pub(crate) realm_id: [u8; 32],
    pub(crate) collateral_mint: [u8; 32],
    pub(crate) token_program: [u8; 32],
    pub(crate) collateral_release: [u8; 32],
    pub(crate) resolution_policy_id: [u8; 32],
    pub(crate) principal_cap_sets: u64,
}

/// One fully authenticated Found37 projection and its unapplied creation plan.
///
/// Series may join additional immutable child facts against these coordinates
/// without reauthenticating the Product graph or Registry records. The Market
/// remains vacant until [`apply_prepared`] is called.
pub(crate) struct PreparedFound {
    creation: CreationPlan,
    pub(crate) realm_id: [u8; 32],
    pub(crate) collateral_mint: [u8; 32],
    pub(crate) token_program: [u8; 32],
    pub(crate) collateral_release: [u8; 32],
    pub(crate) product_record_id: [u8; 32],
    pub(crate) product_id: [u8; 32],
    pub(crate) product: Product,
    pub(crate) runtime: Box<AuthenticatedProductRuntimeV2>,
    pub(crate) resolution_policy_id: [u8; 32],
    pub(crate) manifest_id: [u8; 32],
    pub(crate) release_set_id: [u8; 32],
}

impl PreparedFound {
    /// Candidate Core state authenticated by [`prepare`] but not yet written.
    pub(crate) const fn candidate_state(&self) -> CoreState {
        self.creation.state
    }

    /// Borrow the authenticated candidate without copying its fixed-layout
    /// state through an SBF caller frame.
    pub(crate) const fn candidate_state_ref(&self) -> &CoreState {
        &self.creation.state
    }
}

/// Authenticate the exact readonly ProjectFound36 authority graph and return
/// its future Market projection without acquiring any writable account or
/// applying the prepared creation plan.
#[inline(never)]
pub(crate) fn project(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
    exact_found_request: &[u8],
) -> Result<(), solana_program::program_error::ProgramError> {
    if request.action != Action::Found {
        return Err(CoreSbfError::Instruction.into());
    }
    let frame = FoundAccounts::parse_project(program_id, accounts)?;
    let rent = Rent::get().map_err(|_| CoreSbfError::Creation)?;
    let prepared = prepare_boxed(program_id, &frame, request, &rent)?;
    let receipt = ProjectFoundReceiptV2::new(
        request.market,
        request.generation,
        identity(prepared.realm_id)?,
        identity(prepared.collateral_mint)?,
        identity(prepared.token_program)?,
        identity(prepared.collateral_release)?,
        identity(prepared.product_record_id)?,
        identity(prepared.product_id)?,
        identity(prepared.resolution_policy_id)?,
        identity(prepared.release_set_id)?,
        identity(frame.rent_program.key.to_bytes())?,
        prepared.candidate_state().principal_cap_sets,
        hash(exact_found_request).to_bytes(),
    )
    .map_err(|_| CoreSbfError::Transition)?;
    let bytes = receipt.encode().map_err(|_| CoreSbfError::Transition)?;
    set_return_data(&bytes);
    Ok(())
}

#[inline(never)]
fn prepare_boxed(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
    request: Request,
    rent: &Rent,
) -> Result<Box<PreparedFound>, solana_program::program_error::ProgramError> {
    prepare(program_id, frame, request, rent)
}

#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
) -> Result<(), solana_program::program_error::ProgramError> {
    if request.action != Action::Found {
        return Err(CoreSbfError::Instruction.into());
    }
    process_frame(program_id, accounts, request)
}

#[inline(never)]
fn process_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
) -> Result<(), solana_program::program_error::ProgramError> {
    let frame = FoundAccounts::parse(program_id, accounts)?;
    let rent_account = frame.rent.ok_or(CoreSbfError::AccountFrame)?;
    let rent = Rent::from_account_info(rent_account).map_err(|_| CoreSbfError::Creation)?;
    authenticate_plan_and_apply(program_id, &frame, request, &rent)
}

#[inline(never)]
fn authenticate_plan_and_apply(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
    request: Request,
    rent: &Rent,
) -> Result<(), solana_program::program_error::ProgramError> {
    let prepared = prepare(program_id, frame, request, rent)?;
    apply_prepared(program_id, frame, *prepared)
}

/// Authenticate the complete ordinary Found V3 authority graph and plan the unique
/// Market creation without mutating the vacant Market.
#[inline(never)]
pub(crate) fn prepare(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
    request: Request,
    rent: &Rent,
) -> Result<Box<PreparedFound>, solana_program::program_error::ProgramError> {
    let (references, runtime) = authenticate_prepare_context(program_id, frame, rent)?;
    let common = FoundCommonAccounts::ordinary(frame);
    authenticate_rent_credit(&common, request.generation, references.release_set_id)?;
    let admission = authenticate_role(
        frame.activation_cache,
        frame.registry_program,
        frame.core_program,
        frame.core_programdata,
        identity(frame.registry_program.key.to_bytes())?,
        references.release_set_id,
        Role::Core,
    )?;
    finish_prepare(
        program_id, &common, request, rent, references, runtime, admission,
    )
}

/// Prepare Series Found from the same authority graph while consuming the
/// Core Admission produced by its single canonical Registry role batch.
#[inline(never)]
pub(crate) fn prepare_with_admission(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
    request: Request,
    rent: &Rent,
    admission: Admission,
) -> Result<Box<PreparedFound>, solana_program::program_error::ProgramError> {
    let (references, runtime) = authenticate_prepare_context(program_id, frame, rent)?;
    let common = FoundCommonAccounts::ordinary(frame);
    authenticate_rent_credit(&common, request.generation, references.release_set_id)?;
    finish_prepare(
        program_id, &common, request, rent, references, runtime, admission,
    )
}

/// Prepare the compact projected generic-Found route from facts persisted by
/// an authenticated Custody V2 prestate.
#[inline(never)]
pub(crate) fn prepare_projected_with_admission(
    program_id: &Pubkey,
    frame: &ProjectedFoundAccountsV2<'_, '_>,
    request: Request,
    rent: &Rent,
    admission: Admission,
    authority: ProjectedFoundAuthorityV2,
) -> Result<Box<PreparedFound>, solana_program::program_error::ProgramError> {
    authenticate_projected_found(program_id, frame, rent)?;
    let release_set_id =
        observe_activation_release_set_id(frame.activation_cache, frame.registry_program)?;
    authenticate_projected_immutable_core_release(frame, release_set_id)?;
    let (references, runtime) =
        authenticate_projected_references(frame, rent, release_set_id, authority)?;
    let common = FoundCommonAccounts::projected(frame);
    authenticate_rent_credit(&common, request.generation, release_set_id)?;
    finish_prepare(
        program_id, &common, request, rent, references, runtime, admission,
    )
}

#[inline(never)]
fn authenticate_prepare_context(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
    rent: &Rent,
) -> Result<
    (References, Box<AuthenticatedProductRuntimeV2>),
    solana_program::program_error::ProgramError,
> {
    authenticate_found(program_id, frame, rent)?;
    // The complete Registry activation cache is the exact release projection.
    // Its canonical PDA supplies the release-set identity; Found does not
    // redundantly re-read the release-set raw/staging pair.
    let release_set_id =
        observe_activation_release_set_id(frame.activation_cache, frame.registry_program)?;
    authenticate_immutable_core_release(frame, release_set_id)?;
    let (references, runtime) = authenticate_references(frame, rent, release_set_id)?;
    Ok((references, runtime))
}

#[inline(never)]
fn finish_prepare(
    program_id: &Pubkey,
    frame: &FoundCommonAccounts<'_, '_>,
    request: Request,
    rent: &Rent,
    references: References,
    runtime: Box<AuthenticatedProductRuntimeV2>,
    admission: Admission,
) -> Result<Box<PreparedFound>, solana_program::program_error::ProgramError> {
    let projection = Box::new(PreparedFound {
        realm_id: references.realm_id,
        collateral_mint: references.collateral_mint,
        token_program: references.token_program,
        collateral_release: references.collateral_release,
        product_record_id: references.product_record_id,
        product_id: references.product_id,
        product: references.product,
        runtime,
        resolution_policy_id: references.resolution_policy_id,
        manifest_id: references.manifest_id,
        release_set_id: references.release_set_id,
        creation: plan_found(program_id, frame, request, references, admission, rent)?,
    });
    Ok(projection)
}

/// Apply one already-authenticated Market creation plan exactly once.
#[inline(never)]
pub(crate) fn apply_prepared(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
    prepared: PreparedFound,
) -> Result<(), solana_program::program_error::ProgramError> {
    let plan = prepared.creation;
    apply_creation(
        program_id,
        &FoundCommonAccounts::ordinary(frame),
        plan.state_seeds,
        plan.bump,
        plan.state,
        plan.rent_top_up,
    )?;
    Ok(())
}

/// Apply one compact projected creation plan after all later-stage checks pass.
#[inline(never)]
pub(crate) fn apply_projected_prepared(
    program_id: &Pubkey,
    frame: &ProjectedFoundAccountsV2<'_, '_>,
    prepared: PreparedFound,
) -> Result<(), solana_program::program_error::ProgramError> {
    let plan = prepared.creation;
    apply_creation(
        program_id,
        &FoundCommonAccounts::projected(frame),
        plan.state_seeds,
        plan.bump,
        plan.state,
        plan.rent_top_up,
    )
}

#[inline(never)]
fn plan_found(
    program_id: &Pubkey,
    frame: &FoundCommonAccounts<'_, '_>,
    request: Request,
    references: References,
    admission: dclutch_market_core_codec::Admission,
    rent: &Rent,
) -> Result<CreationPlan, solana_program::program_error::ProgramError> {
    let market_identity = MarketIdentity {
        market_id: identity(frame.market.key.to_bytes())?,
        realm_id: identity(references.realm_id)?,
        product_record: identity(references.product_record_id)?,
        product_id: identity(references.product_id)?,
        resolution_policy: identity(references.resolution_policy_id)?,
        capability_manifest: identity(references.manifest_id)?,
        selected_release_set: identity(references.release_set_id)?,
        registry_program: identity(frame.registry_program.key.to_bytes())?,
        generation: request.generation,
    };
    if request.market != market_identity.market_id {
        return Err(CoreSbfError::Reference.into());
    }
    let state_seeds = MarketCoreStateSeedsV2::new(market_identity);
    let (expected_market, bump) =
        Pubkey::find_program_address(&state_seeds.as_slices(), program_id);
    if frame.market.key != &expected_market
        || frame.market.owner != &system_program::ID
        || frame.market.data_len() != 0
    {
        return Err(CoreSbfError::Market.into());
    }
    let semantic_frame = FoundingFrame {
        realm: Realm {
            realm_id: identity(references.realm_id)?,
            collateral_mint: identity(references.collateral_mint)?,
            token_program: identity(references.token_program)?,
            collateral_release: identity(references.collateral_release)?,
        },
        product: references.product,
        identity: market_identity,
        core_admission: admission,
        principal_cap_sets: references.principal_cap_sets,
        quote: FoundingQuote {
            market_rent: rent.minimum_balance(STATE_BYTES),
        },
        accounts: FoundingAccounts {
            payer_lamports: frame.payer.lamports(),
            rent_credit: identity(frame.rent_credit.key.to_bytes())?,
            market: VacantAccount {
                address: identity(frame.market.key.to_bytes())?,
                lamports: frame.market.lamports(),
                system_owned: true,
                data_empty: true,
                executable: false,
            },
        },
    };
    let result = found(request, semantic_frame).map_err(|_| CoreSbfError::Transition)?;
    Ok(CreationPlan {
        state_seeds,
        bump,
        state: result.state,
        rent_top_up: result.plan.market.rent_top_up,
    })
}

#[inline(never)]
fn authenticate_references(
    frame: &FoundAccounts<'_, '_>,
    rent: &Rent,
    expected_release_set_id: [u8; 32],
) -> Result<(References, Box<AuthenticatedProductRuntimeV2>), CoreSbfError> {
    let registry = frame.registry_program.key;
    let realm_data = frame
        .realm_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if realm_data.len() != REALM_BYTES {
        return Err(CoreSbfError::Reference);
    }
    let (realm_id, realm_bytes) = authenticate_content_addressed_record(
        registry,
        frame.realm_raw,
        frame.realm_staging,
        rent,
        REALM_SCHEMA_RELEASE_ID_V1,
        &realm_data,
    )?;
    let realm = RealmV1::decode(realm_bytes).map_err(|_| CoreSbfError::Reference)?;

    let runtime = Box::new(authenticate_product_runtime_v2(
        registry,
        rent,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: frame.product_raw,
                staging: frame.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: frame.result_domain_raw,
                staging: frame.result_domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: frame.portfolio_raw,
                staging: frame.portfolio_staging,
            },
        },
    )?);
    let product_record_id = runtime.product_record.content_digest.to_bytes();
    let product_id = runtime.product_id.to_bytes();
    let product = project_core_product_v2(*runtime)?;

    let basis = authenticate_product_basis_v3(
        registry,
        rent,
        *runtime,
        FinalizedRecordFrameV2 {
            raw: frame.linked_basis_raw,
            staging: frame.linked_basis_staging,
        },
    )
    .map_err(|_| CoreSbfError::Reference)?;

    let resolution_data = frame
        .resolution_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if resolution_data.len() != SOURCE_MATERIAL_V3_BYTES {
        return Err(CoreSbfError::Reference);
    }
    let (resolution_policy_id, resolution_bytes) = authenticate_content_addressed_record(
        registry,
        frame.resolution_raw,
        frame.resolution_staging,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        &resolution_data,
    )?;
    let material =
        SourceMaterialV3::decode(resolution_bytes).map_err(|_| CoreSbfError::Reference)?;
    material
        .authenticate_product_record(
            SourceContentId::new(product_record_id).map_err(|_| CoreSbfError::Reference)?,
        )
        .map_err(|_| CoreSbfError::Reference)?;

    let (source_spec_id, source_spec) = authenticate_source_record(
        registry,
        frame.source_spec_raw,
        frame.source_spec_staging,
        rent,
        SOURCE_SPEC_SCHEMA_ID_V1,
        SOURCE_SPEC_BYTES,
        SourceSpecV1::decode,
    )?;
    let (capacity_profile_id, capacity_profile) = authenticate_source_record(
        registry,
        frame.capacity_profile_raw,
        frame.capacity_profile_staging,
        rent,
        SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
        SOURCE_CAPACITY_PROFILE_BYTES,
        SourceCapacityProfileV1::decode,
    )?;
    let floor = match material.principal_policy() {
        SourcePrincipalPolicyV1::ExplicitlyUnbounded => {
            authenticate_absent_optional_record(
                registry,
                frame.manipulation_floor_raw,
                frame.manipulation_floor_staging,
                MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            )?;
            None
        }
        SourcePrincipalPolicyV1::BoundedByFloor(_) => {
            let (floor_id, floor) = authenticate_source_record(
                registry,
                frame.manipulation_floor_raw,
                frame.manipulation_floor_staging,
                rent,
                MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
                MANIPULATION_FLOOR_V1_BYTES,
                ManipulationFloorV1::decode,
            )?;
            Some((floor_id, floor))
        }
    };
    let principal_cap_sets = material
        .derive_principal_cap_sets(
            source_spec_id,
            source_spec,
            capacity_profile_id,
            capacity_profile,
            floor,
            SourceContentId::new(*realm.collateral_mint()).map_err(|_| CoreSbfError::Reference)?,
            basis.payout_scale,
        )
        .map_err(|_| CoreSbfError::Reference)?
        .to_sets();
    if principal_cap_sets == 0 {
        return Err(CoreSbfError::Reference);
    }

    let manifest_data = frame
        .manifest_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let (manifest_id, manifest_bytes) = authenticate_content_addressed_record(
        registry,
        frame.manifest_raw,
        frame.manifest_staging,
        rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest_data,
    )?;
    CapabilityManifestV1::decode(manifest_bytes).map_err(|_| CoreSbfError::Reference)?;

    Ok((
        References {
            realm_id,
            collateral_mint: *realm.collateral_mint(),
            token_program: *realm.token_program(),
            collateral_release: *realm.collateral_adapter_release_id(),
            product_record_id,
            product_id,
            product,
            resolution_policy_id,
            manifest_id,
            release_set_id: expected_release_set_id,
            principal_cap_sets,
        },
        runtime,
    ))
}

#[inline(never)]
fn authenticate_projected_references(
    frame: &ProjectedFoundAccountsV2<'_, '_>,
    rent: &Rent,
    release_set_id: [u8; 32],
    authority: ProjectedFoundAuthorityV2,
) -> Result<(References, Box<AuthenticatedProductRuntimeV2>), CoreSbfError> {
    if authority.principal_cap_sets == 0 {
        return Err(CoreSbfError::Reference);
    }
    let registry = frame.registry_program.key;
    let runtime = Box::new(authenticate_product_runtime_v2(
        registry,
        rent,
        ProductRuntimeFrameV2 {
            product: FinalizedRecordFrameV2 {
                raw: frame.product_raw,
                staging: frame.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: frame.result_domain_raw,
                staging: frame.result_domain_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: frame.portfolio_raw,
                staging: frame.portfolio_staging,
            },
        },
    )?);
    let product_record_id = runtime.product_record.content_digest.to_bytes();
    let product_id = runtime.product_id.to_bytes();
    let product = project_core_product_v2(*runtime)?;

    let manifest_data = frame
        .manifest_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let (manifest_id, manifest_bytes) = authenticate_content_addressed_record(
        registry,
        frame.manifest_raw,
        frame.manifest_staging,
        rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest_data,
    )?;
    CapabilityManifestV1::decode(manifest_bytes).map_err(|_| CoreSbfError::Reference)?;

    Ok((
        References {
            realm_id: authority.realm_id,
            collateral_mint: authority.collateral_mint,
            token_program: authority.token_program,
            collateral_release: authority.collateral_release,
            product_record_id,
            product_id,
            product,
            resolution_policy_id: authority.resolution_policy_id,
            manifest_id,
            release_set_id,
            principal_cap_sets: authority.principal_cap_sets,
        },
        runtime,
    ))
}

#[inline(never)]
fn observe_activation_release_set_id(
    cache: &AccountInfo<'_>,
    registry: &AccountInfo<'_>,
) -> Result<[u8; 32], CoreSbfError> {
    if cache.owner != registry.key || cache.is_signer || cache.is_writable || cache.executable {
        return Err(CoreSbfError::Release);
    }
    let data = cache.try_borrow_data().map_err(|_| CoreSbfError::Release)?;
    let view =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| CoreSbfError::Release)?;
    let release_set_id = view
        .execution_release_set_id()
        .map_err(|_| CoreSbfError::Release)?
        .to_bytes();
    let expected =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set_id], registry.key).0;
    if cache.key != &expected {
        return Err(CoreSbfError::Release);
    }
    Ok(release_set_id)
}

fn authenticate_source_record<T, E>(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    schema: [u8; 32],
    expected_width: usize,
    decode: fn(&[u8]) -> Result<T, E>,
) -> Result<(SourceContentId, T), CoreSbfError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if data.len() != expected_width {
        return Err(CoreSbfError::Reference);
    }
    let (digest, bytes) =
        authenticate_content_addressed_record(registry, raw, staging, rent, schema, &data)?;
    let identity = SourceContentId::new(digest).map_err(|_| CoreSbfError::Reference)?;
    let value = decode(bytes).map_err(|_| CoreSbfError::Reference)?;
    Ok((identity, value))
}

/// Authenticate the canonical absence witness used by an explicitly-unbounded
/// Source policy. Both optional floor coordinates are deterministic vacant
/// System accounts for the zero content ID; no unrelated floor may ride unused.
fn authenticate_absent_optional_record<'info>(
    registry: &Pubkey,
    raw: &AccountInfo<'info>,
    staging: &AccountInfo<'info>,
    schema: [u8; 32],
) -> Result<(), CoreSbfError> {
    let absent = [0_u8; 32];
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &absent], registry).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &absent], registry).0;
    for (account, expected) in [(raw, expected_raw), (staging, expected_staging)] {
        if account.key != &expected
            || account.owner != &system_program::ID
            || account.data_len() != 0
            || account.executable
            || account.is_signer
            || account.is_writable
        {
            return Err(CoreSbfError::FinalizedRecord);
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_rent_credit(
    frame: &FoundCommonAccounts<'_, '_>,
    generation: u64,
    release_set: [u8; 32],
) -> Result<(), CoreSbfError> {
    if frame.rent_credit.owner != frame.rent_program.key {
        return Err(CoreSbfError::RentCredit);
    }
    let bytes = frame
        .rent_credit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit = LifecycleRentCreditV2::decode(&bytes).map_err(|_| CoreSbfError::RentCredit)?;
    if credit.market().to_bytes() != frame.market.key.to_bytes()
        || credit.release_set().to_bytes() != release_set
        || credit.generation() != generation
    {
        return Err(CoreSbfError::RentCredit);
    }
    let market = credit.market().to_bytes();
    let generation_bytes = credit.generation().to_le_bytes();
    let bump = [credit.pda_bump()];
    let expected = Pubkey::create_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            &market,
            &generation_bytes,
            &bump,
        ],
        frame.rent_program.key,
    )
    .map_err(|_| CoreSbfError::RentCredit)?;
    if frame.rent_credit.key != &expected {
        return Err(CoreSbfError::RentCredit);
    }
    Ok(())
}

#[inline(never)]
fn apply_creation(
    program_id: &Pubkey,
    frame: &FoundCommonAccounts<'_, '_>,
    state_seeds: MarketCoreStateSeedsV2,
    bump: u8,
    state: CoreState,
    rent_top_up: u64,
) -> Result<(), solana_program::program_error::ProgramError> {
    if rent_top_up > 0 {
        invoke(
            &transfer(frame.payer.key, frame.market.key, rent_top_up),
            &[
                frame.payer.clone(),
                frame.market.clone(),
                frame.system.clone(),
            ],
        )
        .map_err(|_| CoreSbfError::Creation)?;
    }
    let seeds = state_seeds.as_slices();
    let [
        domain,
        realm,
        product_record,
        product_id,
        resolution,
        manifest,
        release,
        registry,
        generation,
    ] = seeds;
    let bump_seed = [bump];
    let signer: [&[u8]; 10] = [
        domain,
        realm,
        product_record,
        product_id,
        resolution,
        manifest,
        release,
        registry,
        generation,
        &bump_seed,
    ];
    invoke_signed(
        &allocate(
            frame.market.key,
            u64::try_from(STATE_BYTES).map_err(|_| CoreSbfError::Arithmetic)?,
        ),
        &[frame.market.clone(), frame.system.clone()],
        &[&signer],
    )
    .map_err(|_| CoreSbfError::Creation)?;
    invoke_signed(
        &assign(frame.market.key, program_id),
        &[frame.market.clone(), frame.system.clone()],
        &[&signer],
    )
    .map_err(|_| CoreSbfError::Creation)?;
    let encoded = state.encode().map_err(|_| CoreSbfError::Commit)?;
    {
        let mut data = frame
            .market
            .try_borrow_mut_data()
            .map_err(|_| CoreSbfError::Commit)?;
        if data.len() != STATE_BYTES {
            return Err(CoreSbfError::Commit.into());
        }
        data.copy_from_slice(&encoded);
    }
    let post = frame
        .market
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Commit)?;
    if frame.market.owner != program_id
        || frame.market.data_len() != STATE_BYTES
        || CoreState::decode(&post) != Ok(state)
    {
        return Err(CoreSbfError::Commit.into());
    }
    Ok(())
}
