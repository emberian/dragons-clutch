//! Exact finalized-record Found transition and prepaid Market creation.

use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_market_core_codec::{
    Action, CoreState, FoundingAccounts, FoundingFrame, FoundingQuote, MarketCoreStateSeedsV1,
    MarketIdentity, Product, Realm, Request, Role, STATE_BYTES, VacantAccount, found,
};
use dclutch_product_contract::{
    product::{
        INSTANCE_BYTES, InstanceV1, PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        PRODUCT_TERMS_SCHEMA_RELEASE_ID_V1, TERMS_BYTES, TermsV1,
    },
    result_domain::{
        FINITE_RESULT_DOMAIN_BYTES, FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
        FINITE_RESULT_DOMAIN_SCHEMA_RELEASE_ID_V1, FiniteResultDomainV1,
    },
};
use dclutch_realm_contract::{REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_release_set_contract::{
    EXECUTION_RELEASE_SET_BYTES_V1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
    ExecutionReleaseSetV1, ExecutionRoleV1,
};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RentCreditV1};
use dclutch_source_contract::{
    SOURCE_MATERIAL_BYTES, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SourceMaterialViewV1,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hashv,
    program::{invoke, invoke_signed},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};

use crate::{
    CoreSbfError,
    frame::FoundAccounts,
    records::{authenticate_content_addressed_record, authenticate_finalized_record},
    release::{authenticate_role, identity},
};

struct References {
    realm_id: [u8; 32],
    realm: RealmV1,
    product_id: [u8; 32],
    product: Product,
    result_domain_id: [u8; 32],
    resolution_policy_id: [u8; 32],
    manifest_id: [u8; 32],
    release_set_id: [u8; 32],
}

struct CreationPlan {
    state_seeds: MarketCoreStateSeedsV1,
    bump: u8,
    state: CoreState,
    rent_top_up: u64,
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
    let frame = FoundAccounts::parse(program_id, accounts)?;
    let rent = Rent::from_account_info(frame.rent).map_err(|_| CoreSbfError::Creation)?;
    let references = authenticate_references(&frame, &rent)?;
    authenticate_rent_credit(&frame)?;
    let admission = authenticate_role(
        frame.activation_cache,
        frame.registry_program,
        frame.core_program,
        frame.core_programdata,
        references.release_set_id,
        Role::Core,
    )?;
    authenticate_release_record(&frame, &rent, references.release_set_id, admission.selected)?;
    let plan = plan_found(program_id, &frame, request, references, admission, &rent)?;
    apply_creation(
        program_id,
        &frame,
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
        product_id: identity(references.product_id)?,
        result_domain: identity(references.result_domain_id)?,
        resolution_policy: identity(references.resolution_policy_id)?,
        capability_manifest: identity(references.manifest_id)?,
        selected_release_set: identity(references.release_set_id)?,
        generation: request.generation,
    };
    if request.market != market_identity.market_id {
        return Err(CoreSbfError::Reference.into());
    }
    let state_seeds = MarketCoreStateSeedsV1::new(market_identity);
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
) -> Result<References, CoreSbfError> {
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

    let instance_data = frame
        .instance_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if instance_data.len() != INSTANCE_BYTES {
        return Err(CoreSbfError::Reference);
    }
    let (product_id, instance_bytes) = authenticate_content_addressed_record(
        registry,
        frame.instance_raw,
        frame.instance_staging,
        rent,
        PRODUCT_INSTANCE_SCHEMA_RELEASE_ID_V1,
        &instance_data,
    )?;
    let instance = InstanceV1::decode(instance_bytes).map_err(|_| CoreSbfError::Reference)?;

    let terms_data = frame
        .terms_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if terms_data.len() != TERMS_BYTES {
        return Err(CoreSbfError::Reference);
    }
    authenticate_finalized_record(
        registry,
        frame.terms_raw,
        frame.terms_staging,
        rent,
        PRODUCT_TERMS_SCHEMA_RELEASE_ID_V1,
        instance.terms_id().to_bytes(),
        &terms_data,
    )?;
    let terms = TermsV1::decode(&terms_data).map_err(|_| CoreSbfError::Reference)?;
    if instance.capacity_profile_id() != terms.capacity_profile_id()
        || instance.partition_cell_count() != terms.partition_cell_count()
    {
        return Err(CoreSbfError::Reference);
    }

    let domain_data = frame
        .domain_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if domain_data.len() != FINITE_RESULT_DOMAIN_BYTES {
        return Err(CoreSbfError::Reference);
    }
    let (_, domain_bytes) = authenticate_content_addressed_record(
        registry,
        frame.domain_raw,
        frame.domain_staging,
        rent,
        FINITE_RESULT_DOMAIN_SCHEMA_RELEASE_ID_V1,
        &domain_data,
    )?;
    let domain = FiniteResultDomainV1::decode(domain_bytes).map_err(|_| CoreSbfError::Reference)?;
    let result_domain_id =
        hashv(&[FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1, &[0], domain_bytes]).to_bytes();
    if instance.result_domain_id().to_bytes() != result_domain_id
        || instance.partition_cell_count() != u32::from(domain.outcome_count())
    {
        return Err(CoreSbfError::Reference);
    }

    let resolution_data = frame
        .resolution_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if resolution_data.len() != SOURCE_MATERIAL_BYTES {
        return Err(CoreSbfError::Reference);
    }
    let (resolution_policy_id, resolution_bytes) = authenticate_content_addressed_record(
        registry,
        frame.resolution_raw,
        frame.resolution_staging,
        rent,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
        &resolution_data,
    )?;
    let material =
        SourceMaterialViewV1::decode(resolution_bytes).map_err(|_| CoreSbfError::Reference)?;
    if material
        .product_instance_id()
        .map_err(|_| CoreSbfError::Reference)?
        .to_bytes()
        != product_id
        || material
            .result_domain()
            .map_err(|_| CoreSbfError::Reference)?
            != domain
    {
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
    ExecutionReleaseSetV1::decode(release_bytes).map_err(|_| CoreSbfError::Reference)?;

    Ok(References {
        realm_id,
        realm,
        product_id,
        product: Product {
            product_id: identity(product_id)?,
            result_domain: identity(result_domain_id)?,
            claim_basis: identity(instance.claim_basis_id().to_bytes())?,
            capacity_profile: identity(instance.capacity_profile_id().content_id().to_bytes())?,
            compiler_release: identity(terms.semantic_release_id().to_bytes())?,
            outcome_count: u32::from(domain.outcome_count()),
        },
        result_domain_id,
        resolution_policy_id,
        manifest_id,
        release_set_id,
    })
}

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
    state_seeds: MarketCoreStateSeedsV1,
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
        product,
        result_domain,
        resolution,
        manifest,
        release,
        generation,
    ] = seeds;
    let bump_seed = [bump];
    let signer: [&[u8]; 9] = [
        domain,
        realm,
        product,
        result_domain,
        resolution,
        manifest,
        release,
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
