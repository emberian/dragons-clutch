//! Concrete SBF account and lamport-custody boundary for recurring Series.
//!
//! This module is compiled only by the non-production Product/Series
//! laboratory. It authenticates the account forms that already have frozen
//! semantics: registered-Series and funding-state PDAs, exact stored/current
//! rent coverage, and five physically distinct zero-data System-owned lamport
//! vaults. It also supplies exact-delta System transfers into and out of those
//! vaults.
//!
//! It intentionally does not implement a dispatch entry point yet. A complete
//! action still needs typed central-registry, SourcePlane V3, collateral V2,
//! failure-admission, and runtime-liveness receipt adapters. Consequently all
//! Source/Series capability tuples remain disabled and none of these mutation
//! helpers is reachable from instruction data.

use crate::accounts::{expect_pda, require, require_count, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, create_pda_account, require_creatable, require_system_program,
    transfer_data, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_collateral_adapter_v2::{
    admit_realm_collateral_account_v2, admit_realm_collateral_mint_v2, prepare_custody_creation_v2,
    BoundRealmCollateralV2, CustodyBindingV2, CustodyCreationPlanV2, CustodyInitializationStepV2,
    Id as CollateralId, RuntimeAccountViewV2, TokenAccountRoleV2,
};
use clutch_product_series::{
    AuthenticatedSeriesFundingAuthorityV1, ComponentDebitV1, ContentId,
    EvidenceOnlyRecoveryPolicyV1, FixedCodec, MarketGenesisProfileV2, NativeClaimBasisV1,
    PriceMeasurePolicyV1, ProductTemplateV4, RegistryCapabilityProjectionV2,
    SeriesAttachmentPlanV1, SeriesFundingComponentV1, SeriesFundingQuoteV1,
    SeriesFundingRequirementsV1, SeriesFundingStateV1, SeriesFundingTermsV2,
    SeriesFundingTermsV2Id, SeriesPlanV5, SeriesPlanV5Id, SERIES_FUNDING_COMPONENT_COUNT,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::product_series::{
    SeriesFundingAccountV1, SeriesRegistryAccountV1, SERIES_FUNDING_ACCOUNT_BYTES_V1,
    SERIES_REGISTRY_ACCOUNT_BYTES_V1,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

/// Closed number of physical Series funding compartments.
pub const SERIES_CUSTODY_COUNT_V1: usize = SERIES_FUNDING_COMPONENT_COUNT;

/// Exact read-only Product/Series artifact count used by registration and
/// every later transition that reconstructs the immutable join.
pub const SERIES_ARTIFACT_ACCOUNT_COUNT_V1: usize = 9;

/// SeriesPlan V5 artifact account index.
pub const IX_SERIES_ARTIFACT_PLAN: usize = 0;
/// SeriesFundingTerms V2 artifact account index.
pub const IX_SERIES_ARTIFACT_FUNDING_TERMS: usize = 1;
/// ProductTemplate V4 artifact account index.
pub const IX_SERIES_ARTIFACT_TEMPLATE: usize = 2;
/// NativeClaimBasis V1 artifact account index.
pub const IX_SERIES_ARTIFACT_BASIS: usize = 3;
/// EvidenceOnlyRecoveryPolicy V1 artifact account index.
pub const IX_SERIES_ARTIFACT_RECOVERY: usize = 4;
/// PriceMeasurePolicy V1 artifact account index.
pub const IX_SERIES_ARTIFACT_PRICE_POLICY: usize = 5;
/// MarketGenesisProfile V2 artifact account index.
pub const IX_SERIES_ARTIFACT_GENESIS: usize = 6;
/// SeriesFundingQuote V1 artifact account index.
pub const IX_SERIES_ARTIFACT_QUOTE: usize = 7;
/// SeriesAttachmentPlan V1 artifact account index.
pub const IX_SERIES_ARTIFACT_ATTACHMENT: usize = 8;

/// Exact decoded immutable bodies selected by one registered Series.
///
/// Construction is private to [`authenticate_series_artifact_accounts`], so a
/// caller cannot obtain this type from claimed IDs or caller-built projections.
/// It proves account owner, read-only role, exact body length, content-derived
/// PDA, hostile decode, recomputed typed identity, and all body-to-body
/// references. It deliberately does not prove central-registry provenance.
#[derive(Debug)]
pub struct AuthenticatedSeriesArtifactsV1 {
    /// Finite recurring Series plan.
    pub series: Box<SeriesPlanV5>,
    /// Immutable principal/refund/sink/mint/program ownership.
    pub funding_terms: Box<SeriesFundingTermsV2>,
    /// Reusable relative Product semantics.
    pub template: Box<ProductTemplateV4>,
    /// Exact canonical payout partition.
    pub basis: Box<NativeClaimBasisV1>,
    /// Exact evidence-only recovery schedule.
    pub recovery: Box<EvidenceOnlyRecoveryPolicyV1>,
    /// Exact quantized price-measure semantics.
    pub price_policy: Box<PriceMeasurePolicyV1>,
    /// Immutable Realm/Profile and venue semantics.
    pub genesis: Box<MarketGenesisProfileV2>,
    /// Per-occurrence component funding quote.
    pub quote: Box<SeriesFundingQuoteV1>,
    /// Operational attachment identities excluded from Market identity.
    pub attachment: Box<SeriesAttachmentPlanV1>,
}

impl AuthenticatedSeriesArtifactsV1 {
    /// Apply the pure complete registry join after the adapter has separately
    /// authenticated the authoritative registry release and selector mapping.
    ///
    /// Success here does not authenticate a caller-built projection; it merely
    /// avoids duplicating any artifact equality or capability rule in SBF.
    pub fn validate_registry_projection(
        &self,
        projection: &RegistryCapabilityProjectionV2,
    ) -> Outcome<SeriesFundingRequirementsV1> {
        self.series
            .validate_bindings(
                &self.template,
                &self.basis,
                &self.recovery,
                &self.price_policy,
                &self.genesis,
                &self.attachment,
                projection,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.funding_terms
            .validate_bindings(
                &self.series,
                &self.template,
                &self.basis,
                &self.recovery,
                &self.price_policy,
                &self.genesis,
                projection,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        self.quote
            .validate_recovery_binding(&self.recovery)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        SeriesFundingRequirementsV1::derive(&self.series, &self.attachment, &self.quote)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
    }
}

fn require_product_artifact_metadata(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    kind: ArtifactKind,
    digest: ContentId,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(
        account.data_len() == kind.exact_len(),
        ClutchError::WrongDataLength,
    )?;
    expect_pda(
        account.key,
        seeds::product_artifact_pda(program_id, kind.byte(), &digest.bytes()),
        None,
    )
}

fn decode_product_artifact<T: FixedCodec>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    kind: ArtifactKind,
    digest: ContentId,
) -> Outcome<Box<T>> {
    require_product_artifact_metadata(program_id, account, kind, digest)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    T::decode(&data)
        .map(Box::new)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn decode_basis_artifact(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected: clutch_product_series::NativeClaimBasisId,
) -> Outcome<Box<NativeClaimBasisV1>> {
    require_product_artifact_metadata(
        program_id,
        account,
        ArtifactKind::NativeClaimBasisV1,
        expected.content_id(),
    )?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mut value = Box::new(NativeClaimBasisV1::ZEROED);
    NativeClaimBasisV1::decode_into(&data, &mut value)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        value
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == expected,
        ClutchError::MismatchedState,
    )?;
    Ok(value)
}

/// Authenticate and decode the exact nine immutable Product/Series artifacts.
///
/// The account order is frozen by the `IX_SERIES_ARTIFACT_*` constants. Every
/// dependent expected ID comes from an already-authenticated parent body, not
/// from another instruction field. The two root IDs are the registration
/// payload's Series and FundingTerms claims. Central registry and runtime
/// release accounts are intentionally absent from this mechanical segment.
pub fn authenticate_series_artifact_accounts(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    expected_series: SeriesPlanV5Id,
    expected_funding_terms: SeriesFundingTermsV2Id,
) -> Outcome<AuthenticatedSeriesArtifactsV1> {
    require_count(accounts, SERIES_ARTIFACT_ACCOUNT_COUNT_V1)?;
    expected_series
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expected_funding_terms
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let series = decode_product_artifact::<SeriesPlanV5>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_PLAN],
        ArtifactKind::SeriesPlanV5,
        expected_series.content_id(),
    )?;
    require(
        series
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == expected_series,
        ClutchError::MismatchedState,
    )?;
    let funding_terms = decode_product_artifact::<SeriesFundingTermsV2>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_FUNDING_TERMS],
        ArtifactKind::SeriesFundingTermsV2,
        expected_funding_terms.content_id(),
    )?;
    require(
        funding_terms
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == expected_funding_terms
            && funding_terms.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;

    let template = decode_product_artifact::<ProductTemplateV4>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_TEMPLATE],
        ArtifactKind::ProductTemplateV4,
        series.product_template_id.content_id(),
    )?;
    require(
        template
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == series.product_template_id,
        ClutchError::MismatchedState,
    )?;
    let genesis = decode_product_artifact::<MarketGenesisProfileV2>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_GENESIS],
        ArtifactKind::MarketGenesisProfileV2,
        series.market_genesis_profile_id.content_id(),
    )?;
    require(
        genesis
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == series.market_genesis_profile_id,
        ClutchError::MismatchedState,
    )?;
    let attachment = decode_product_artifact::<SeriesAttachmentPlanV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_ATTACHMENT],
        ArtifactKind::SeriesAttachmentPlanV1,
        series.attachment_plan_id.content_id(),
    )?;
    require(
        attachment
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == series.attachment_plan_id,
        ClutchError::MismatchedState,
    )?;

    let basis = decode_basis_artifact(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_BASIS],
        template.native_claim_basis_id,
    )?;
    let recovery = decode_product_artifact::<EvidenceOnlyRecoveryPolicyV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_RECOVERY],
        ArtifactKind::EvidenceOnlyRecoveryPolicyV1,
        template.evidence_only_recovery_policy_id.content_id(),
    )?;
    require(
        recovery
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == template.evidence_only_recovery_policy_id,
        ClutchError::MismatchedState,
    )?;
    let price_policy = decode_product_artifact::<PriceMeasurePolicyV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_PRICE_POLICY],
        ArtifactKind::PriceMeasurePolicyV1,
        genesis.price_measure_policy_id.content_id(),
    )?;
    require(
        price_policy
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == genesis.price_measure_policy_id,
        ClutchError::MismatchedState,
    )?;
    let quote = decode_product_artifact::<SeriesFundingQuoteV1>(
        program_id,
        &accounts[IX_SERIES_ARTIFACT_QUOTE],
        ArtifactKind::SeriesFundingQuoteV1,
        attachment.funding_quote_id.content_id(),
    )?;
    require(
        quote
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == attachment.funding_quote_id,
        ClutchError::MismatchedState,
    )?;

    template
        .validate_bindings(&basis, &recovery)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    genesis
        .validate_bindings(&basis, &price_policy)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    quote
        .validate_recovery_binding(&recovery)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    SeriesFundingRequirementsV1::derive(&series, &attachment, &quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    Ok(AuthenticatedSeriesArtifactsV1 {
        series,
        funding_terms,
        template,
        basis,
        recovery,
        price_policy,
        genesis,
        quote,
        attachment,
    })
}

/// Exact accounted balances for all five physical custody pairs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesCustodyBalancesV1 {
    /// Native balance in each zero-data System-owned PDA.
    pub lamports: [u64; SERIES_CUSTODY_COUNT_V1],
    /// Collateral atoms in each release-selected segregated vault.
    pub collateral_atoms: [u64; SERIES_CUSTODY_COUNT_V1],
}

fn add(left: u64, right: u64) -> Outcome<u64> {
    left.checked_add(right)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

fn sub(left: u64, right: u64) -> Outcome<u64> {
    left.checked_sub(right)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))
}

fn component_from_index(index: usize) -> Outcome<SeriesFundingComponentV1> {
    match index {
        0 => Ok(SeriesFundingComponentV1::MarketCore),
        1 => Ok(SeriesFundingComponentV1::RecoveryReserve),
        2 => Ok(SeriesFundingComponentV1::SourceWork),
        3 => Ok(SeriesFundingComponentV1::LiquidityFacility),
        4 => Ok(SeriesFundingComponentV1::WrapperSet),
        _ => Err(ClutchError::NonCanonical.into()),
    }
}

/// Derive exact physical balances from the state-owned principal/donation
/// facts. Consumed allocation is already absent from `remaining_principal`.
pub fn accounted_custody_balances(
    state: &SeriesFundingStateV1,
) -> Outcome<SeriesCustodyBalancesV1> {
    state
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut value = SeriesCustodyBalancesV1 {
        lamports: [0; SERIES_CUSTODY_COUNT_V1],
        collateral_atoms: [0; SERIES_CUSTODY_COUNT_V1],
    };
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        let component = state.components[index];
        value.lamports[index] = add(
            component.remaining_principal.lamports,
            component.donations.lamports,
        )?;
        value.collateral_atoms[index] = add(
            component.remaining_principal.collateral_atoms,
            component.donations.collateral_atoms,
        )?;
        index += 1;
    }
    Ok(value)
}

fn collateral_id(key: &Pubkey) -> CollateralId {
    CollateralId::from_bytes(key.to_bytes())
}

fn require_collateral_program(
    account: &AccountInfo<'_>,
    bound: BoundRealmCollateralV2,
) -> Outcome<()> {
    require(
        collateral_id(account.key) == bound.release().token_program,
        ClutchError::WrongTokenProgram,
    )?;
    require(account.executable, ClutchError::WrongTokenProgram)?;
    require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    require(!account.is_signer, ClutchError::MismatchedState)
}

fn require_series_collateral_authority(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    authority: &AccountInfo<'_>,
) -> Outcome<()> {
    expect_pda(
        authority.key,
        seeds::series_collateral_authority_pda(program_id, &series.bytes()),
        None,
    )?;
    require(!authority.is_writable, ClutchError::UnexpectedWritable)?;
    require(!authority.is_signer, ClutchError::MismatchedState)?;
    require(!authority.executable, ClutchError::ExecutableAccount)?;
    require(authority.data_is_empty(), ClutchError::WrongDataLength)
}

fn series_collateral_binding(
    program_id: &Pubkey,
    bound: BoundRealmCollateralV2,
    series: SeriesPlanV5Id,
    component: SeriesFundingComponentV1,
    vault: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
) -> Outcome<CustodyBindingV2> {
    series
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_series_collateral_authority(program_id, series, authority)?;
    expect_pda(
        vault.key,
        seeds::series_collateral_vault_pda(program_id, &series.bytes(), component as u8),
        None,
    )?;
    require(vault.is_writable, ClutchError::NotWritable)?;
    require(!vault.executable, ClutchError::ExecutableAccount)?;
    let binding = CustodyBindingV2 {
        account: collateral_id(vault.key),
        owner_authority: collateral_id(authority.key),
        semantic_owner: CollateralId::from_bytes(series.bytes()),
        compartment: u16::from(component as u8) + 1,
        owner_guard: bound.release().owner_guard,
        owner_authority_is_program_derived: true,
    };
    binding
        .validate(bound.release())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(binding)
}

fn runtime_account_view<'a>(account: &AccountInfo<'_>, data: &'a [u8]) -> RuntimeAccountViewV2<'a> {
    RuntimeAccountViewV2 {
        key: collateral_id(account.key),
        owner_program: collateral_id(account.owner),
        data,
        is_signer: account.is_signer,
        is_writable: account.is_writable,
        executable: account.executable,
    }
}

/// Authenticate the Realm-selected mint and all five release-selected Series
/// collateral vaults against the exact state-owned balances.
///
/// `bound` must itself come from the separately authenticated
/// Realm/Profile/policy/release/loader seam. This helper adds only AccountInfo,
/// PDA, hostile token-byte, semantic-owner, compartment, and exact-balance
/// authentication.
pub fn authenticate_series_collateral_custody(
    program_id: &Pubkey,
    bound: BoundRealmCollateralV2,
    series: SeriesPlanV5Id,
    mint: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
    vaults: &[AccountInfo<'_>],
    funding: &SeriesFundingAccountV1,
    rent: &RentParameters,
) -> Outcome<()> {
    require(
        vaults.len() == SERIES_CUSTODY_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    funding.validate()?;
    require(
        funding.state.series_plan_id == series,
        ClutchError::MismatchedState,
    )?;
    let expected = accounted_custody_balances(&funding.state)?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    admit_realm_collateral_mint_v2(bound, runtime_account_view(mint, &mint_data))
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        let component = component_from_index(index)?;
        let binding = series_collateral_binding(
            program_id,
            bound,
            series,
            component,
            &vaults[index],
            authority,
        )?;
        let data = vaults[index]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observation = admit_realm_collateral_account_v2(
            bound,
            runtime_account_view(&vaults[index], &data),
            TokenAccountRoleV2::SegregatedVault(binding),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let stored_rent = funding.collateral_vault_rent_principal_lamports[index];
        let current_rent = rent.minimum_balance(vaults[index].data_len())?;
        require(
            observation.amount_atoms == expected.collateral_atoms[index]
                && vaults[index].lamports() >= stored_rent
                && vaults[index].lamports() >= current_rent,
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    Ok(())
}

fn require_custody_creation_plan(
    plan: CustodyCreationPlanV2,
    token_program: &AccountInfo<'_>,
    vault: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    authority: &AccountInfo<'_>,
) -> Outcome<()> {
    require(
        plan.token_program == collateral_id(token_program.key)
            && plan.account == collateral_id(vault.key)
            && plan.mint == collateral_id(mint.key)
            && plan.owner_authority == collateral_id(authority.key),
        ClutchError::MismatchedState,
    )?;
    require(
        plan.step_count != 0 && usize::from(plan.step_count) <= plan.steps.len(),
        ClutchError::MismatchedState,
    )
}

fn invoke_custody_initialization<'a>(
    plan: CustodyCreationPlanV2,
    vault: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Outcome<()> {
    let mut index = 0usize;
    while index < usize::from(plan.step_count) {
        match plan.steps[index] {
            CustodyInitializationStepV2::None => return Err(ClutchError::MismatchedState.into()),
            CustodyInitializationStepV2::InitializeImmutableOwner { account, data } => {
                require(
                    account == collateral_id(vault.key),
                    ClutchError::MismatchedState,
                )?;
                let instruction = Instruction::new_with_bytes(
                    *token_program.key,
                    &data,
                    vec![AccountMeta::new(*vault.key, false)],
                );
                invoke(&instruction, &[vault.clone(), token_program.clone()])
                    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
            }
            CustodyInitializationStepV2::InitializeAccount3 {
                account,
                mint: planned_mint,
                owner_authority: _,
                data,
            } => {
                require(
                    account == collateral_id(vault.key) && planned_mint == collateral_id(mint.key),
                    ClutchError::MismatchedState,
                )?;
                let instruction = Instruction::new_with_bytes(
                    *token_program.key,
                    &data,
                    vec![
                        AccountMeta::new(*vault.key, false),
                        AccountMeta::new_readonly(*mint.key, false),
                    ],
                );
                invoke(
                    &instruction,
                    &[vault.clone(), mint.clone(), token_program.clone()],
                )
                .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
            }
        }
        index += 1;
    }
    Ok(())
}

/// Create and admit one release-selected Series collateral vault while
/// preserving exact payer ownership of its rent principal.
///
/// Any lamports sent to the predictable address before creation are first
/// PDA-signed into the authenticated neutral sink. The payer then supplies the
/// complete current rent principal; prefunding never becomes a discount or a
/// refund claim. The returned principal must be persisted in the matching
/// `SeriesFundingAccountV1` component slot before activation can commit.
#[allow(clippy::too_many_arguments)]
pub fn create_series_collateral_vault<'a>(
    program_id: &Pubkey,
    bound: BoundRealmCollateralV2,
    series: SeriesPlanV5Id,
    component: SeriesFundingComponentV1,
    payer: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    rent: &RentParameters,
) -> Outcome<u64> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(neutral_sink.is_writable, ClutchError::NotWritable)?;
    require(!neutral_sink.executable, ClutchError::ExecutableAccount)?;
    require_creatable(vault)?;
    require_system_program(system_program)?;
    require_collateral_program(token_program, bound)?;
    let binding =
        series_collateral_binding(program_id, bound, series, component, vault, authority)?;
    let plan = prepare_custody_creation_v2(bound, binding)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_custody_creation_plan(plan, token_program, vault, mint, authority)?;
    require(
        payer.key != vault.key
            && payer.key != neutral_sink.key
            && vault.key != neutral_sink.key
            && vault.key != mint.key
            && vault.key != authority.key
            && vault.key != token_program.key,
        ClutchError::AccountAlias,
    )?;

    let component_seed = [component as u8];
    let series_seed = series.bytes();
    let (_, bump) = seeds::series_collateral_vault_pda(program_id, &series_seed, component as u8);
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        seeds::SEED_SERIES_COLLATERAL_VAULT_V1,
        &series_seed,
        &component_seed,
        &bump_seed,
    ];

    let prefund = vault.lamports();
    if prefund != 0 {
        let sink_before = neutral_sink.lamports();
        let sweep = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(prefund),
            vec![
                AccountMeta::new(*vault.key, true),
                AccountMeta::new(*neutral_sink.key, false),
            ],
        );
        invoke_signed(
            &sweep,
            &[vault.clone(), neutral_sink.clone(), system_program.clone()],
            &[signer_seeds],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
        require(
            vault.lamports() == 0 && neutral_sink.lamports() == add(sink_before, prefund)?,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
    }

    let account_bytes = usize::from(plan.account_bytes);
    let rent_principal_lamports = rent.minimum_balance(account_bytes)?;
    require(rent_principal_lamports != 0, ClutchError::MismatchedState)?;
    let payer_before = payer.lamports();
    let fund = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(rent_principal_lamports),
        vec![
            AccountMeta::new(*payer.key, true),
            AccountMeta::new(*vault.key, false),
        ],
    );
    invoke(
        &fund,
        &[payer.clone(), vault.clone(), system_program.clone()],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        payer.lamports() == sub(payer_before, rent_principal_lamports)?
            && vault.lamports() == rent_principal_lamports,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;

    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(account_bytes),
        vec![AccountMeta::new(*vault.key, true)],
    );
    invoke_signed(
        &allocate,
        &[vault.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(token_program.key),
        vec![AccountMeta::new(*vault.key, true)],
    );
    invoke_signed(
        &assign,
        &[vault.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        vault.data_len() == account_bytes
            && vault.owner == token_program.key
            && vault.lamports() == rent_principal_lamports,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;

    invoke_custody_initialization(plan, vault, mint, token_program)?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    admit_realm_collateral_mint_v2(bound, runtime_account_view(mint, &mint_data))
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let vault_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observation = admit_realm_collateral_account_v2(
        bound,
        runtime_account_view(vault, &vault_data),
        TokenAccountRoleV2::SegregatedVault(binding),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        observation.amount_atoms == 0 && vault.lamports() == rent_principal_lamports,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    Ok(rent_principal_lamports)
}

fn require_program_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: usize,
    writable: bool,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    if writable {
        require(account.is_writable, ClutchError::NotWritable)?;
    } else {
        require(!account.is_writable, ClutchError::UnexpectedWritable)?;
    }
    require(
        account.data_len() == exact_len,
        ClutchError::WrongDataLength,
    )
}

fn require_rent_coverage(
    account: &AccountInfo<'_>,
    stored_principal: u64,
    rent: &RentParameters,
) -> Outcome<()> {
    let current_minimum = rent.minimum_balance(account.data_len())?;
    require(stored_principal != 0, ClutchError::MismatchedState)?;
    require(
        account.lamports() >= stored_principal && account.lamports() >= current_minimum,
        ClutchError::MismatchedState,
    )
}

fn credit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let next = add(account.lamports(), amount)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = next;
    Ok(())
}

fn debit_lamports(account: &AccountInfo<'_>, amount: u64) -> Outcome<()> {
    let next = sub(account.lamports(), amount)?;
    let mut lamports = account
        .try_borrow_mut_lamports()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    **lamports = next;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn create_series_program_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    exact_len: usize,
    rent_principal_lamports: u64,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require(neutral_sink.is_writable, ClutchError::NotWritable)?;
    require(!neutral_sink.executable, ClutchError::ExecutableAccount)?;
    require(
        payer.key != target.key && payer.key != neutral_sink.key && target.key != neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    require_system_program(system_program)?;
    let minimum = rent.minimum_balance(exact_len)?;
    require(
        rent_principal_lamports == minimum,
        ClutchError::MismatchedState,
    )?;
    create_pda_account(
        program_id,
        payer,
        target,
        system_program,
        rent,
        exact_len,
        signer_seeds,
    )?;
    let surplus = sub(target.lamports(), rent_principal_lamports)?;
    let sink_before = neutral_sink.lamports();
    if surplus != 0 {
        debit_lamports(target, surplus)?;
        credit_lamports(neutral_sink, surplus)?;
    }
    require(
        target.lamports() == rent_principal_lamports
            && neutral_sink.lamports() == add(sink_before, surplus)?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )
}

/// Create and encode one immutable registered-Series PDA after a higher typed
/// adapter has authenticated its full registry/artifact join.
///
/// Predictable-address prefunding is donation, not a rent discount. The payer
/// still supplies the exact rent shortfall; any preexisting surplus is moved
/// atomically to the already-authenticated FundingTerms V2 neutral sink.
#[allow(clippy::too_many_arguments)]
pub fn create_series_registry_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    value: SeriesRegistryAccountV1,
) -> Outcome<()> {
    value.validate()?;
    let (address, bump) = seeds::series_registry_pda(program_id, &value.series_plan_id.bytes());
    expect_pda(target.key, (address, bump), Some(value.stored_bump))?;
    let bump_seed = [bump];
    let series_seed = value.series_plan_id.bytes();
    create_series_program_account(
        program_id,
        payer,
        target,
        neutral_sink,
        system_program,
        rent,
        SERIES_REGISTRY_ACCOUNT_BYTES_V1,
        value.rent_principal_lamports,
        &[seeds::SEED_SERIES_REGISTRY_V1, &series_seed, &bump_seed],
    )?;
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    Ok(())
}

/// Create and encode one mutable funding PDA after exact activation principal,
/// collateral-vault, liveness, and registry joins have produced its pure state.
#[allow(clippy::too_many_arguments)]
pub fn create_series_funding_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    target: &AccountInfo<'a>,
    neutral_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    value: SeriesFundingAccountV1,
) -> Outcome<()> {
    value.validate()?;
    let (address, bump) =
        seeds::series_funding_pda(program_id, &value.state.series_plan_id.bytes());
    expect_pda(target.key, (address, bump), Some(value.stored_bump))?;
    let bump_seed = [bump];
    let series_seed = value.state.series_plan_id.bytes();
    create_series_program_account(
        program_id,
        payer,
        target,
        neutral_sink,
        system_program,
        rent,
        SERIES_FUNDING_ACCOUNT_BYTES_V1,
        value.rent_principal_lamports,
        &[seeds::SEED_SERIES_FUNDING_V1, &series_seed, &bump_seed],
    )?;
    let mut data = target
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    value.encode(&mut data)?;
    Ok(())
}

/// Authenticate an immutable registered-Series account, including its exact
/// PDA, program owner, codec, and stored/current rent coverage.
pub fn read_series_registry_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
    rent: &RentParameters,
) -> Outcome<SeriesRegistryAccountV1> {
    require_program_account(program_id, account, SERIES_REGISTRY_ACCOUNT_BYTES_V1, false)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesRegistryAccountV1::decode(&data)?;
    require(
        value.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        account.key,
        seeds::series_registry_pda(program_id, &expected_series.bytes()),
        Some(value.stored_bump),
    )?;
    require_rent_coverage(account, value.rent_principal_lamports, rent)?;
    Ok(value)
}

/// Authenticate a mutable Series-funding account and join it to its exact
/// quote before returning the decoded state.
pub fn read_series_funding_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_series: SeriesPlanV5Id,
    quote: &clutch_product_series::SeriesFundingQuoteV1,
    rent: &RentParameters,
) -> Outcome<SeriesFundingAccountV1> {
    require_program_account(program_id, account, SERIES_FUNDING_ACCOUNT_BYTES_V1, true)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let value = SeriesFundingAccountV1::decode(&data)?;
    require(
        value.state.series_plan_id == expected_series,
        ClutchError::MismatchedState,
    )?;
    value
        .state
        .validate_against_quote(quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    expect_pda(
        account.key,
        seeds::series_funding_pda(program_id, &expected_series.bytes()),
        Some(value.stored_bump),
    )?;
    require_rent_coverage(account, value.rent_principal_lamports, rent)?;
    Ok(value)
}

/// Write one already-validated atomic successor state back to its authenticated
/// account wrapper without changing rent ownership or framing.
pub fn write_series_funding_state(
    account: &AccountInfo<'_>,
    current: SeriesFundingAccountV1,
    next: SeriesFundingStateV1,
) -> Outcome<()> {
    require(
        current.state.series_plan_id == next.series_plan_id
            && current.state.funding_terms_id == next.funding_terms_id
            && current.state.funding_quote_id == next.funding_quote_id
            && current.state.instance_count == next.instance_count,
        ClutchError::MismatchedState,
    )?;
    next.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let updated = SeriesFundingAccountV1 {
        state: next,
        ..current
    };
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    updated.encode(&mut data)?;
    Ok(())
}

fn require_lamport_vault_metadata(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    index: usize,
    account: &AccountInfo<'_>,
) -> Outcome<()> {
    let component = component_from_index(index)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(!account.executable, ClutchError::ExecutableAccount)?;
    require(
        *account.owner == SYSTEM_PROGRAM_ID && account.data_is_empty(),
        ClutchError::WrongProgramOwner,
    )?;
    expect_pda(
        account.key,
        seeds::series_lamport_vault_pda(program_id, &series.bytes(), component as u8),
        None,
    )?;
    Ok(())
}

/// Authenticate all five lamport custody PDAs and exact equality with the
/// state-owned balance. A surplus must first be observed as donation; a
/// shortfall always refuses.
pub fn authenticate_lamport_custody(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    vaults: &[AccountInfo<'_>],
    expected: &SeriesCustodyBalancesV1,
) -> Outcome<()> {
    require(
        vaults.len() == SERIES_CUSTODY_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        require_lamport_vault_metadata(program_id, series, index, &vaults[index])?;
        require(
            vaults[index].lamports() == expected.lamports[index],
            ClutchError::MismatchedState,
        )?;
        index += 1;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct AuthenticatedLamportDonationV1 {
    series_plan_id: SeriesPlanV5Id,
    funding_quote_id: clutch_product_series::SeriesFundingQuoteId,
    component: SeriesFundingComponentV1,
    amount: ComponentDebitV1,
}

impl AuthenticatedSeriesFundingAuthorityV1 for AuthenticatedLamportDonationV1 {
    fn authenticate_donation(
        &self,
        state: &SeriesFundingStateV1,
        quote: &SeriesFundingQuoteV1,
        component: SeriesFundingComponentV1,
        amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        if state.series_plan_id != self.series_plan_id
            || quote.id()? != self.funding_quote_id
            || component != self.component
            || amount != self.amount
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Observe one positive lamport-vault surplus and apply exactly that donation
/// to a copy of the pure funding state.
///
/// The private authority value is constructible only after exact PDA/owner/data
/// authentication and an actual-balance delta. It cannot authorize activation,
/// Clock, or occurrence fulfillment through the trait's default-deny methods.
pub fn observe_lamport_donation(
    program_id: &Pubkey,
    state: SeriesFundingStateV1,
    quote: &SeriesFundingQuoteV1,
    component: SeriesFundingComponentV1,
    vault: &AccountInfo<'_>,
) -> Outcome<SeriesFundingStateV1> {
    state
        .validate_against_quote(quote)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require_lamport_vault_metadata(program_id, state.series_plan_id, component.index(), vault)?;
    let accounted = accounted_custody_balances(&state)?.lamports[component.index()];
    let actual = vault.lamports();
    let delta = sub(actual, accounted)?;
    require(delta != 0, ClutchError::MismatchedState)?;
    let amount = ComponentDebitV1 {
        lamports: delta,
        collateral_atoms: 0,
    };
    let authority = AuthenticatedLamportDonationV1 {
        series_plan_id: state.series_plan_id,
        funding_quote_id: state.funding_quote_id,
        component,
        amount,
    };
    let mut next = state;
    next.add_donation(&authority, quote, component, amount)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        accounted_custody_balances(&next)?.lamports[component.index()] == actual,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    Ok(next)
}

/// Fund all five lamport custody PDAs from one authenticated payer with exact
/// component post-deltas. Existing balances are preserved as separately
/// observed activation donations; they never reduce the payer debit.
pub fn fund_lamport_custody<'a>(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    vaults: &[AccountInfo<'a>],
    principal: [ComponentDebitV1; SERIES_CUSTODY_COUNT_V1],
) -> Outcome<()> {
    require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?;
    require(
        vaults.len() == SERIES_CUSTODY_COUNT_V1,
        ClutchError::AccountCount,
    )?;
    let mut index = 0usize;
    while index < SERIES_CUSTODY_COUNT_V1 {
        require_lamport_vault_metadata(program_id, series, index, &vaults[index])?;
        require(payer.key != vaults[index].key, ClutchError::AccountAlias)?;
        index += 1;
    }
    index = 0;
    while index < SERIES_CUSTODY_COUNT_V1 {
        let amount = principal[index].lamports;
        let before = vaults[index].lamports();
        let expected_after = add(before, amount)?;
        if amount != 0 {
            let transfer = Instruction::new_with_bytes(
                SYSTEM_PROGRAM_ID,
                &transfer_data(amount),
                vec![
                    AccountMeta::new(*payer.key, true),
                    AccountMeta::new(*vaults[index].key, false),
                ],
            );
            invoke(
                &transfer,
                &[payer.clone(), vaults[index].clone(), system_program.clone()],
            )
            .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
        }
        require(
            vaults[index].lamports() == expected_after,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        index += 1;
    }
    Ok(())
}

/// Transfer an exact amount out of one zero-data component vault, signed only
/// by its canonical PDA, and verify both source and destination deltas.
#[allow(clippy::too_many_arguments)]
pub fn transfer_from_lamport_custody<'a>(
    program_id: &Pubkey,
    series: SeriesPlanV5Id,
    component: SeriesFundingComponentV1,
    vault: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    amount: u64,
) -> Outcome<()> {
    let index = component.index();
    require_lamport_vault_metadata(program_id, series, index, vault)?;
    require(destination.is_writable, ClutchError::NotWritable)?;
    require(!destination.executable, ClutchError::ExecutableAccount)?;
    require(vault.key != destination.key, ClutchError::AccountAlias)?;
    require_system_program(system_program)?;
    let source_before = vault.lamports();
    let destination_before = destination.lamports();
    let source_after = sub(source_before, amount)?;
    let destination_after = add(destination_before, amount)?;
    if amount != 0 {
        let transfer = Instruction::new_with_bytes(
            SYSTEM_PROGRAM_ID,
            &transfer_data(amount),
            vec![
                AccountMeta::new(*vault.key, true),
                AccountMeta::new(*destination.key, false),
            ],
        );
        let component_seed = [component as u8];
        let (_, bump) =
            seeds::series_lamport_vault_pda(program_id, &series.bytes(), component as u8);
        let series_seed = series.bytes();
        let bump_seed = [bump];
        invoke_signed(
            &transfer,
            &[vault.clone(), destination.clone(), system_program.clone()],
            &[&[
                seeds::SEED_SERIES_LAMPORT_VAULT_V1,
                &series_seed,
                &component_seed,
                &bump_seed,
            ]],
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    }
    require(
        vault.lamports() == source_after && destination.lamports() == destination_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )
}

/// Close one program-owned Series account with payer rent and donation surplus
/// kept separate. The exact stored principal goes to the FundingTerms V2
/// lamport refund account; every other lamport goes to its distinct neutral
/// sink. Callers must authenticate the account codec/PDA and closed lifecycle
/// before entering this mechanical primitive.
pub fn close_series_program_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    principal_refund: &AccountInfo<'_>,
    neutral_sink: &AccountInfo<'_>,
    rent_principal_lamports: u64,
) -> Outcome<()> {
    require(account.owner == program_id, ClutchError::WrongProgramOwner)?;
    require(account.is_writable, ClutchError::NotWritable)?;
    require(principal_refund.is_writable, ClutchError::NotWritable)?;
    require(neutral_sink.is_writable, ClutchError::NotWritable)?;
    require(
        !account.executable && !principal_refund.executable && !neutral_sink.executable,
        ClutchError::ExecutableAccount,
    )?;
    require(
        account.key != principal_refund.key
            && account.key != neutral_sink.key
            && principal_refund.key != neutral_sink.key,
        ClutchError::AccountAlias,
    )?;
    let held = account.lamports();
    let donation = sub(held, rent_principal_lamports)?;
    let refund_before = principal_refund.lamports();
    let sink_before = neutral_sink.lamports();
    let refund_after = add(refund_before, rent_principal_lamports)?;
    let sink_after = add(sink_before, donation)?;
    credit_lamports(principal_refund, rent_principal_lamports)?;
    credit_lamports(neutral_sink, donation)?;
    debit_lamports(account, held)?;
    require(
        account.lamports() == 0
            && principal_refund.lamports() == refund_after
            && neutral_sink.lamports() == sink_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    data.fill(0);
    Ok(())
}
