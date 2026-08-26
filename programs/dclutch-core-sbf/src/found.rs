//! Exact finalized-record Found transition and prepaid Market creation.

use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_market_core_codec::{
    Action, CoreState, FoundingAccounts, FoundingFrame, FoundingQuote, MarketCoreStateSeedsV2,
    MarketIdentity, Product, ProjectFoundReceiptV1, Realm, Request, Role, STATE_BYTES,
    VacantAccount, found,
};
use dclutch_product_runtime_v2_svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV2};
use dclutch_realm_contract::{REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_release_set_contract::{
    EXECUTION_RELEASE_SET_BYTES_V1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
    ExecutionReleaseSetV1, ExecutionRoleV1,
};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RentCreditV1};
use dclutch_source_contract::{
    ContentId as SourceContentId, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SOURCE_MATERIAL_V2_BYTES,
    SourceMaterialV2,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    program::{invoke, invoke_signed, set_return_data},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};
use alloc::boxed::Box;
use dclutch_product_runtime_v2_svm_reader::AuthenticatedProductRuntimeV2;

use crate::{
    CoreSbfError,
    frame::FoundAccounts,
    infrastructure::{authenticate_found, authenticate_immutable_core_release},
    product_runtime_v2::{authenticate_product_runtime_v2, project_core_product_v2},
    records::{authenticate_content_addressed_record, authenticate_finalized_record},
    release::{authenticate_role, identity},
};

struct References {
    realm_id: [u8; 32],
    realm: RealmV1,
    product_record_id: [u8; 32],
    product_id: [u8; 32],
    product: Product,
    resolution_policy_id: [u8; 32],
    manifest_id: [u8; 32],
    release_set_id: [u8; 32],
}

struct CreationPlan {
    state_seeds: MarketCoreStateSeedsV2,
    bump: u8,
    state: CoreState,
    rent_top_up: u64,
}

/// One fully authenticated Found31 projection and its unapplied creation plan.
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
}

/// Authenticate the exact Found31 authority graph and return its future
/// Market projection without acquiring any writable account or applying the
/// prepared creation plan.
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
    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Creation)?;
    let prepared = prepare_boxed(program_id, &frame, request, &rent)?;
    let receipt = ProjectFoundReceiptV1::new(
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
    Ok(Box::new(prepare(program_id, frame, request, rent)?))
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
    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Creation)?;
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
    apply_prepared(program_id, frame, prepared)
}

/// Authenticate the complete Found31 authority graph and plan the unique
/// Market creation without mutating the vacant Market.
#[inline(never)]
pub(crate) fn prepare(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
    request: Request,
    rent: &Rent,
) -> Result<PreparedFound, solana_program::program_error::ProgramError> {
    authenticate_found(program_id, frame, rent)?;
    // The release-set bytes are observed only to address the immutable Registry cache.
    // They are not accepted as a finalized record until after the current Core release
    // has been authenticated directly from that now-immutable Registry observation.
    let release_set_id = observe_release_set_id(frame)?;
    authenticate_immutable_core_release(frame, release_set_id)?;
    let (references, runtime) = authenticate_references(frame, rent, release_set_id)?;
    authenticate_rent_credit(frame)?;
    let admission = authenticate_role(
        frame.activation_cache,
        frame.registry_program,
        frame.core_program,
        frame.core_programdata,
        identity(frame.registry_program.key.to_bytes())?,
        references.release_set_id,
        Role::Core,
    )?;
    authenticate_release_record(frame, rent, references.release_set_id, admission.selected)?;
    let projection = PreparedFound {
        realm_id: references.realm_id,
        collateral_mint: *references.realm.collateral_mint(),
        token_program: *references.realm.token_program(),
        collateral_release: *references.realm.collateral_adapter_release_id(),
        product_record_id: references.product_record_id,
        product_id: references.product_id,
        product: references.product,
        runtime,
        resolution_policy_id: references.resolution_policy_id,
        manifest_id: references.manifest_id,
        release_set_id: references.release_set_id,
        creation: plan_found(program_id, frame, request, references, admission, rent)?,
    };
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
        frame,
        plan.state_seeds,
        plan.bump,
        plan.state,
        plan.rent_top_up,
    )?;
    Ok(())
}

#[inline(never)]
fn plan_found(
    program_id: &Pubkey,
    frame: &FoundAccounts<'_, '_>,
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
            collateral_mint: identity(*references.realm.collateral_mint())?,
            token_program: identity(*references.realm.token_program())?,
            collateral_release: identity(*references.realm.collateral_adapter_release_id())?,
        },
        product: references.product,
        identity: market_identity,
        core_admission: admission,
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

    let resolution_data = frame
        .resolution_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if resolution_data.len() != SOURCE_MATERIAL_V2_BYTES {
        return Err(CoreSbfError::Reference);
    }
    let (resolution_policy_id, resolution_bytes) = authenticate_content_addressed_record(
        registry,
        frame.resolution_raw,
        frame.resolution_staging,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        &resolution_data,
    )?;
    SourceMaterialV2::decode(resolution_bytes)
        .and_then(|material| {
            material.authenticate_product_record(SourceContentId::new(product_record_id)?)
        })
        .map_err(|_| CoreSbfError::Reference)?;

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

    let release_data = frame
        .release_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if release_data.len() != EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(CoreSbfError::Reference);
    }
    let (release_set_id, release_bytes) = authenticate_content_addressed_record(
        registry,
        frame.release_raw,
        frame.release_staging,
        rent,
        EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
        &release_data,
    )?;
    if release_set_id != expected_release_set_id {
        return Err(CoreSbfError::Release);
    }
    ExecutionReleaseSetV1::decode(release_bytes).map_err(|_| CoreSbfError::Reference)?;

    Ok((References {
        realm_id,
        realm,
        product_record_id,
        product_id,
        product,
        resolution_policy_id,
        manifest_id,
        release_set_id,
    }, runtime))
}

#[inline(never)]
fn observe_release_set_id(frame: &FoundAccounts<'_, '_>) -> Result<[u8; 32], CoreSbfError> {
    let data = frame
        .release_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if data.len() != EXECUTION_RELEASE_SET_BYTES_V1 {
        return Err(CoreSbfError::Reference);
    }
    Ok(hash(&data).to_bytes())
}

#[inline(never)]
fn authenticate_release_record(
    frame: &FoundAccounts<'_, '_>,
    rent: &Rent,
    release_set_id: [u8; 32],
    selected: dclutch_market_core_codec::ReleaseSet,
) -> Result<(), CoreSbfError> {
    let data = frame
        .release_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    authenticate_finalized_record(
        frame.registry_program.key,
        frame.release_raw,
        frame.release_staging,
        rent,
        EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
        release_set_id,
        &data,
    )?;
    let exact = ExecutionReleaseSetV1::decode(&data).map_err(|_| CoreSbfError::Release)?;
    for (index, role) in [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Trading,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ]
    .into_iter()
    .enumerate()
    {
        let projected = selected
            .bindings
            .get(index)
            .copied()
            .ok_or(CoreSbfError::Release)?;
        let expected = exact.binding(role);
        if projected.program.to_bytes() != expected.program().to_bytes()
            || projected.artifact_release.to_bytes() != expected.artifact_release().to_bytes()
        {
            return Err(CoreSbfError::Release);
        }
    }
    Ok(())
}

#[inline(never)]
fn authenticate_rent_credit(frame: &FoundAccounts<'_, '_>) -> Result<(), CoreSbfError> {
    if frame.rent_credit.owner != frame.rent_program.key {
        return Err(CoreSbfError::RentCredit);
    }
    let bytes = frame
        .rent_credit
        .try_borrow_data()
        .map_err(|_| CoreSbfError::RentCredit)?;
    let credit = RentCreditV1::decode(&bytes).map_err(|_| CoreSbfError::RentCredit)?;
    let authority = credit.refund_authority().to_bytes();
    let bump = [credit.pda_bump()];
    let expected = Pubkey::create_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, &authority, &bump],
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
    frame: &FoundAccounts<'_, '_>,
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
