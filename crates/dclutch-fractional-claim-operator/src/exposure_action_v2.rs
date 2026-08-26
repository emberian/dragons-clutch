//! Exposure-bound Fractional Token effects and canonical terminal Claims candidate.
//!
//! This module intentionally stops before claiming terminal settlement. The
//! family-neutral ProductBasis terminal kernel emits the sole SignedDelta
//! candidate and exact payout, while the still-missing Claims-owned terminal
//! route must compose that mutation with Custody before Trading commits root.

use dclutch_claims_svm::{
    CallerRole,
    product_basis_terminal_v3::{
        ProductBasisTerminalInputV3, encode_product_basis_terminal_signed_delta_v3,
    },
    signed_delta_v3::{SignedDeltaV3, plan_bytes},
};
use dclutch_fractional_claim_contract::{FractionalExposureActionV2, FractionalExposureRequestV2};
use dclutch_fractional_claim_kernel::{
    ExposureShardDivisionV2, FractionalExposureTermsV2, check_fractional_exposure_bundle_v2,
    divide_exposure_shards_v2,
};
use dclutch_market_core_codec::RetirementReceiptV1;
use dclutch_rent_contract::lifecycle_v2::{
    CloseLifecycleRentCreditV2, LifecycleAccountIdV2, LifecycleClosePlanV2,
    LifecycleRentCloseReceiptV2, LifecycleRentCreditV2,
};
use dclutch_representation_composition_v3_kernel::{
    CompositionExposureBundleV3, RecordAdmissionV3,
};
use dclutch_token_svm::{
    TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, Token2022BehaviorProfileV2, TokenAccount,
    TokenBehaviorSelectionV2,
};
use sha2::{Digest, Sha256};
use solana_program::{instruction::Instruction, pubkey::Pubkey};
use spl_token_2022_interface::{
    extension::permissioned_burn::instruction as permissioned_burn_instruction,
    instruction as token_instruction,
};

use crate::{Error, FractionalTokenAccountSnapshotV1, Result};

/// Finalized-record evidence for one terms-selected TokenBehaviorV2 record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalTokenBehaviorRecordAdmissionV2 {
    /// Terms-selected schema identity.
    pub selected_schema_id: [u8; 32],
    /// Finalized record schema identity.
    pub finalized_schema_id: [u8; 32],
    /// Terms-selected content identity.
    pub selected_content_digest: [u8; 32],
    /// Finalized record content identity.
    pub finalized_content_digest: [u8; 32],
    /// SHA-256 recomputed over the exact record bytes.
    pub recomputed_content_digest: [u8; 32],
    /// Owner/PDA/staging/rent authentication completed.
    pub record_authenticated: bool,
    /// Realm identity came from the immutable authenticated Market.
    pub market_realm_authenticated: bool,
}

/// Non-forgeable join of V2 terms and one finalized Token behavior selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedFractionalTokenBehaviorV2 {
    selection: TokenBehaviorSelectionV2,
    content_digest: [u8; 32],
}

impl CheckedFractionalTokenBehaviorV2 {
    /// Exact hostile-decoded Token behavior selection.
    pub const fn selection(self) -> TokenBehaviorSelectionV2 {
        self.selection
    }

    /// Finalized selection-record content identity.
    pub const fn content_digest(self) -> [u8; 32] {
        self.content_digest
    }
}

/// Authenticate the terms-selected Token behavior record without caller hints.
pub fn authenticate_fractional_token_behavior_v2(
    terms: FractionalExposureTermsV2<'_>,
    market_realm: [u8; 32],
    selection_bytes: &[u8],
    admission: FractionalTokenBehaviorRecordAdmissionV2,
) -> Result<CheckedFractionalTokenBehaviorV2> {
    if !admission.market_realm_authenticated
        || market_realm == [0; 32]
        || admission.selected_schema_id != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
        || admission.finalized_schema_id != TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2
        || !admission.record_authenticated
        || admission.selected_content_digest == [0; 32]
        || admission.selected_content_digest != terms.token_behavior()
        || admission.selected_content_digest != admission.finalized_content_digest
        || admission.selected_content_digest != admission.recomputed_content_digest
    {
        return Err(Error::Token);
    }
    let selection = TokenBehaviorSelectionV2::decode_for_authenticated_selection(
        selection_bytes,
        market_realm,
        terms.release_set(),
    )
    .map_err(|_| Error::Token)?;
    if selection.token_program() != terms.token_program() {
        return Err(Error::Token);
    }
    Ok(CheckedFractionalTokenBehaviorV2 {
        selection,
        content_digest: admission.selected_content_digest,
    })
}

/// Exact V2 Token-owned pre-state for one action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureTokenObservationV2<'a> {
    /// Fractional root PDA controlling mint, close, and permissioned burn.
    pub root_controller: Pubkey,
    /// Terms-selected shard Mint; absent only for state-only actions.
    pub mint: Option<FractionalTokenAccountSnapshotV1<'a>>,
    /// Request-selected source Token account when active.
    pub source: Option<FractionalTokenAccountSnapshotV1<'a>>,
    /// Request-selected destination Token account when active.
    pub destination: Option<FractionalTokenAccountSnapshotV1<'a>>,
    /// Exact current Mint supply.
    pub pre_supply: u64,
    /// Exact current source amount, or zero when inactive.
    pub pre_source: u64,
    /// Exact current destination amount, or zero when inactive.
    pub pre_destination: u64,
}

/// Exact Token-2022 instruction selected by a V2 action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FractionalExposureTokenEffectV2 {
    /// Terminalize changes no Token-owned state.
    None,
    /// Mint exact denominator-scaled shard atoms.
    Mint(Instruction),
    /// Transfer exact raw same-Mint shard atoms.
    Transfer(Instruction),
    /// Permissioned-burn only the exact whole-denominator multiple.
    Burn(Instruction),
}

/// Checked V2 Token pre/post effect with explicit same-Mint change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalExposureTokenPlanV2 {
    effect: FractionalExposureTokenEffectV2,
    division: Option<ExposureShardDivisionV2>,
    consumed_shards: u64,
    change_shards: u64,
    pre_supply: u64,
    post_supply: u64,
    pre_source: u64,
    post_source: u64,
    pre_destination: u64,
    post_destination: u64,
}

/// One K-ordered shard Mint observed for zero-supply retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureMintSnapshotV2<'a> {
    /// Claims representation coordinate in strict `[0,K)` order.
    pub representation_coordinate: u32,
    /// Exact terms-selected Token-2022 Mint account.
    pub mint: FractionalTokenAccountSnapshotV1<'a>,
}

/// Chain-derived authorities for closing the Fractional producer subtree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureRetirementContextV2 {
    /// Fractional root PDA controlling all K Mints.
    pub root_controller: Pubkey,
    /// Root-bound lifecycle RentCredit receiving Mint lamports.
    pub rent_credit: Pubkey,
    /// Registry-selected current Core program for producer-subtree retirement.
    pub current_core_program: Pubkey,
}

/// Ordered zero-supply Mint closures before canonical lifecycle-Rent closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalExposureRetirementPlanV2 {
    instructions: Vec<Instruction>,
    market: [u8; 32],
    release_set: [u8; 32],
    rent_credit: Pubkey,
    current_core_program: Pubkey,
    post_revision: u64,
}

impl FractionalExposureRetirementPlanV2 {
    /// One exact Token-2022 CloseAccount instruction per K-ordered Mint.
    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    /// Logical Market whose producer subtree is retiring.
    pub const fn market(&self) -> [u8; 32] {
        self.market
    }

    /// Immutable selected release set.
    pub const fn release_set(&self) -> [u8; 32] {
        self.release_set
    }

    /// Root-bound lifecycle RentCredit receiving every Mint's lamports.
    pub const fn rent_credit(&self) -> Pubkey {
        self.rent_credit
    }

    /// Exact root revision after retirement.
    pub const fn post_revision(&self) -> u64 {
        self.post_revision
    }
}

/// Chain-observed lifecycle-Rent state for final producer-subtree closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureRentCloseObservationV2<'a> {
    /// Exact lifecycle RentCredit account.
    pub credit_key: Pubkey,
    /// Exact current lifecycle RentCredit bytes.
    pub credit_bytes: &'a [u8],
    /// Full lamport balance transferred on close.
    pub credit_lamports: u64,
    /// Wallet lamports before refund.
    pub wallet_lamports: u64,
    /// Exact canonical Core producer-subtree retirement receipt.
    pub core_receipt_bytes: &'a [u8],
    /// Current Core program/deployment authentication completed.
    pub current_core_authenticated: bool,
}

/// Canonical lifecycle-Rent request, plan, and immediate receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureRentClosePlanV2 {
    /// Exact request carrying the hostile-decoded Core retirement receipt.
    pub request: CloseLifecycleRentCreditV2,
    /// Canonical full-balance close plan.
    pub plan: LifecycleClosePlanV2,
    /// Canonical immediate Rent receipt.
    pub receipt: LifecycleRentCloseReceiptV2,
}

impl FractionalExposureTokenPlanV2 {
    /// Exact Token-2022 instruction, or none for Terminalize.
    pub const fn effect(&self) -> &FractionalExposureTokenEffectV2 {
        &self.effect
    }

    /// Sole denominator division, when the action burns shards.
    pub const fn division(&self) -> Option<ExposureShardDivisionV2> {
        self.division
    }

    /// Exact raw shard atoms minted, transferred, or burned.
    pub const fn consumed_shards(&self) -> u64 {
        self.consumed_shards
    }

    /// Explicit raw same-Mint source balance not burned.
    pub const fn change_shards(&self) -> u64 {
        self.change_shards
    }

    /// Exact Mint supply before the effect.
    pub const fn pre_supply(&self) -> u64 {
        self.pre_supply
    }

    /// Required Mint supply after the effect.
    pub const fn post_supply(&self) -> u64 {
        self.post_supply
    }

    /// Exact source amount before the effect.
    pub const fn pre_source(&self) -> u64 {
        self.pre_source
    }

    /// Required source amount after the effect.
    pub const fn post_source(&self) -> u64 {
        self.post_source
    }

    /// Exact destination amount before the effect.
    pub const fn pre_destination(&self) -> u64 {
        self.pre_destination
    }

    /// Required destination amount after the effect.
    pub const fn post_destination(&self) -> u64 {
        self.post_destination
    }
}

/// Re-derive the exact Token-2022 effect from authenticated V2 terms and state.
pub fn plan_fractional_exposure_token_effect_v2(
    terms: FractionalExposureTermsV2<'_>,
    request: FractionalExposureRequestV2,
    behavior: CheckedFractionalTokenBehaviorV2,
    observed: FractionalExposureTokenObservationV2<'_>,
) -> Result<FractionalExposureTokenPlanV2> {
    let request = request.bind_terms(terms).map_err(|_| Error::Token)?;
    if behavior.content_digest() != terms.token_behavior()
        || behavior.selection().token_program() != terms.token_program()
    {
        return Err(Error::Token);
    }
    let action = request.action();
    if action == FractionalExposureActionV2::ZeroSupplyRetire {
        return Err(Error::Token);
    }
    if action == FractionalExposureActionV2::Terminalize {
        if observed.mint.is_some()
            || observed.source.is_some()
            || observed.destination.is_some()
            || observed.pre_supply != 0
            || observed.pre_source != 0
            || observed.pre_destination != 0
        {
            return Err(Error::Token);
        }
        return Ok(FractionalExposureTokenPlanV2 {
            effect: FractionalExposureTokenEffectV2::None,
            division: None,
            consumed_shards: 0,
            change_shards: 0,
            pre_supply: 0,
            post_supply: 0,
            pre_source: 0,
            post_source: 0,
            pre_destination: 0,
            post_destination: 0,
        });
    }
    let input = request.input();
    let mint = observed.mint.ok_or(Error::Token)?;
    let expected_mint = terms
        .shard_mint(input.representation_coordinate)
        .map_err(|_| Error::Token)?;
    if mint.key.to_bytes() != expected_mint
        || mint.program_owner.to_bytes() != terms.token_program()
        || observed.root_controller == Pubkey::default()
        || observed.root_controller.to_bytes() == input.owner
    {
        return Err(Error::Token);
    }
    let mint_facts = Token2022BehaviorProfileV2::check_mint(
        terms.token_program(),
        expected_mint,
        mint.data,
        observed.root_controller.to_bytes(),
        observed.pre_supply,
    )
    .map_err(|_| Error::Token)?;
    let decimals = mint_facts.display_decimals();
    let (division, consumed_shards, change_shards) = match action {
        FractionalExposureActionV2::Wrap => (
            None,
            input
                .quantity
                .checked_mul(terms.denominator())
                .ok_or(Error::Token)?,
            0,
        ),
        FractionalExposureActionV2::Transfer => (None, input.quantity, 0),
        FractionalExposureActionV2::WholeUnwrap
        | FractionalExposureActionV2::TerminalRedeem
        | FractionalExposureActionV2::TerminalZeroBurn => {
            let division =
                divide_exposure_shards_v2(terms, input.representation_coordinate, input.quantity)
                    .map_err(|_| Error::Token)?;
            (
                Some(division),
                division.consumed.shard_atoms,
                division.change.shard_atoms,
            )
        }
        FractionalExposureActionV2::Terminalize | FractionalExposureActionV2::ZeroSupplyRetire => {
            return Err(Error::Token);
        }
    };
    let owner = Pubkey::new_from_array(input.owner);
    let token_program = Pubkey::new_from_array(terms.token_program());
    let (effect, post_supply, post_source, post_destination) = match action {
        FractionalExposureActionV2::Wrap => {
            if observed.source.is_some() || observed.pre_source != 0 {
                return Err(Error::Token);
            }
            let destination = checked_holder_v2(
                terms,
                observed.destination,
                input.destination_token_account,
                expected_mint,
                input.owner,
                observed.pre_destination,
            )?;
            let instruction = token_instruction::mint_to_checked(
                &token_program,
                &mint.key,
                &destination.key,
                &observed.root_controller,
                &[],
                consumed_shards,
                decimals,
            )
            .map_err(|_| Error::Token)?;
            (
                FractionalExposureTokenEffectV2::Mint(instruction),
                observed
                    .pre_supply
                    .checked_add(consumed_shards)
                    .ok_or(Error::Token)?,
                0,
                observed
                    .pre_destination
                    .checked_add(consumed_shards)
                    .ok_or(Error::Token)?,
            )
        }
        FractionalExposureActionV2::Transfer => {
            let source = checked_holder_v2(
                terms,
                observed.source,
                input.source_token_account,
                expected_mint,
                input.owner,
                observed.pre_source,
            )?;
            let destination = checked_holder_any_owner_v2(
                terms,
                observed.destination,
                input.destination_token_account,
                expected_mint,
                observed.pre_destination,
            )?;
            if source.key == destination.key {
                return Err(Error::Token);
            }
            let instruction = token_instruction::transfer_checked(
                &token_program,
                &source.key,
                &mint.key,
                &destination.key,
                &owner,
                &[],
                consumed_shards,
                decimals,
            )
            .map_err(|_| Error::Token)?;
            (
                FractionalExposureTokenEffectV2::Transfer(instruction),
                observed.pre_supply,
                observed
                    .pre_source
                    .checked_sub(consumed_shards)
                    .ok_or(Error::Token)?,
                observed
                    .pre_destination
                    .checked_add(consumed_shards)
                    .ok_or(Error::Token)?,
            )
        }
        FractionalExposureActionV2::WholeUnwrap
        | FractionalExposureActionV2::TerminalRedeem
        | FractionalExposureActionV2::TerminalZeroBurn => {
            if observed.destination.is_some() || observed.pre_destination != 0 {
                return Err(Error::Token);
            }
            let source = checked_holder_v2(
                terms,
                observed.source,
                input.source_token_account,
                expected_mint,
                input.owner,
                observed.pre_source,
            )?;
            let instruction = permissioned_burn_instruction::burn_checked(
                &token_program,
                &source.key,
                &mint.key,
                &observed.root_controller,
                &owner,
                &[],
                consumed_shards,
                decimals,
            )
            .map_err(|_| Error::Token)?;
            (
                FractionalExposureTokenEffectV2::Burn(instruction),
                observed
                    .pre_supply
                    .checked_sub(consumed_shards)
                    .ok_or(Error::Token)?,
                observed
                    .pre_source
                    .checked_sub(consumed_shards)
                    .ok_or(Error::Token)?,
                0,
            )
        }
        FractionalExposureActionV2::Terminalize | FractionalExposureActionV2::ZeroSupplyRetire => {
            return Err(Error::Token);
        }
    };
    Ok(FractionalExposureTokenPlanV2 {
        effect,
        division,
        consumed_shards,
        change_shards,
        pre_supply: observed.pre_supply,
        post_supply,
        pre_source: observed.pre_source,
        post_source,
        pre_destination: observed.pre_destination,
        post_destination,
    })
}

/// Close every terms-selected K Mint only after exact zero-supply checks.
pub fn plan_fractional_exposure_retirement_v2(
    terms: FractionalExposureTermsV2<'_>,
    request: FractionalExposureRequestV2,
    behavior: CheckedFractionalTokenBehaviorV2,
    context: FractionalExposureRetirementContextV2,
    mints: &[FractionalExposureMintSnapshotV2<'_>],
) -> Result<FractionalExposureRetirementPlanV2> {
    let request = request.bind_terms(terms).map_err(|_| Error::Token)?;
    if request.action() != FractionalExposureActionV2::ZeroSupplyRetire
        || behavior.content_digest() != terms.token_behavior()
        || behavior.selection().token_program() != terms.token_program()
        || context.root_controller == Pubkey::default()
        || context.rent_credit == Pubkey::default()
        || context.current_core_program == Pubkey::default()
        || context.root_controller == context.rent_credit
        || mints.len() != usize::try_from(terms.representation_width()).map_err(|_| Error::Token)?
    {
        return Err(Error::Token);
    }
    let token_program = Pubkey::new_from_array(terms.token_program());
    let mut instructions = Vec::with_capacity(mints.len());
    for (index, observed) in mints.iter().enumerate() {
        let coordinate = u32::try_from(index).map_err(|_| Error::Token)?;
        let expected_mint = terms.shard_mint(coordinate).map_err(|_| Error::Token)?;
        if observed.representation_coordinate != coordinate
            || observed.mint.key.to_bytes() != expected_mint
            || observed.mint.program_owner != token_program
        {
            return Err(Error::Token);
        }
        Token2022BehaviorProfileV2::check_mint(
            terms.token_program(),
            expected_mint,
            observed.mint.data,
            context.root_controller.to_bytes(),
            0,
        )
        .map_err(|_| Error::Token)?;
        instructions.push(
            token_instruction::close_account(
                &token_program,
                &observed.mint.key,
                &context.rent_credit,
                &context.root_controller,
                &[],
            )
            .map_err(|_| Error::Token)?,
        );
    }
    Ok(FractionalExposureRetirementPlanV2 {
        instructions,
        market: terms.market(),
        release_set: terms.release_set(),
        rent_credit: context.rent_credit,
        current_core_program: context.current_core_program,
        post_revision: request
            .input()
            .expected_revision
            .checked_add(1)
            .ok_or(Error::Rent)?,
    })
}

/// Consume canonical Core retirement and derive the sole lifecycle-Rent close.
pub fn plan_fractional_exposure_rent_close_v2(
    retirement: &FractionalExposureRetirementPlanV2,
    observed: FractionalExposureRentCloseObservationV2<'_>,
) -> Result<FractionalExposureRentClosePlanV2> {
    if observed.credit_key != retirement.rent_credit || retirement.instructions.is_empty() {
        return Err(Error::Rent);
    }
    let credit = LifecycleRentCreditV2::decode(observed.credit_bytes).map_err(|_| Error::Rent)?;
    if credit.market().to_bytes() != retirement.market
        || credit.release_set().to_bytes() != retirement.release_set
    {
        return Err(Error::Rent);
    }
    let core_receipt =
        RetirementReceiptV1::decode(observed.core_receipt_bytes).map_err(|_| Error::Rent)?;
    let request = CloseLifecycleRentCreditV2::new(core_receipt);
    let credit_id =
        LifecycleAccountIdV2::new(observed.credit_key.to_bytes()).map_err(|_| Error::Rent)?;
    let core_id = LifecycleAccountIdV2::new(retirement.current_core_program.to_bytes())
        .map_err(|_| Error::Rent)?;
    let plan = LifecycleClosePlanV2::new(
        credit,
        credit_id,
        core_id,
        observed.current_core_authenticated,
        observed.credit_lamports,
        observed.wallet_lamports,
        request,
    )
    .map_err(|_| Error::Rent)?;
    let receipt = plan.receipt(credit, credit_id).map_err(|_| Error::Rent)?;
    Ok(FractionalExposureRentClosePlanV2 {
        request,
        plan,
        receipt,
    })
}

/// Authenticated terminal inputs outside the wallet request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureTerminalInputV2<'a> {
    /// Full canonical ProductBasis terminal input from chain authentication.
    pub claims: ProductBasisTerminalInputV3<'a>,
    /// Authenticated Fractional root owning the canonical Claims reserve Position.
    pub fractional_root: [u8; 32],
    /// Digest of the exact finalized terminal-coordinate record.
    pub terminal_record_digest: [u8; 32],
}

/// Exact terminal SignedDelta candidate awaiting canonical Claims+CPI execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalExposureTerminalCandidateV2 {
    packet: Vec<u8>,
    request_digest: [u8; 32],
    collateral_atoms: u64,
    division: ExposureShardDivisionV2,
}

impl FractionalExposureTerminalCandidateV2 {
    /// Canonical family-neutral SignedDelta packet.
    pub fn packet(&self) -> &[u8] {
        &self.packet
    }

    /// SHA-256 of the exact Fractional V2 request.
    pub const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }

    /// Exact exposure-derived collateral payout; zero is explicit.
    pub const fn collateral_atoms(&self) -> u64 {
        self.collateral_atoms
    }

    /// Sole quotient/remainder result binding the Token burn to Claims debit.
    pub const fn division(&self) -> ExposureShardDivisionV2 {
        self.division
    }
}

/// Derive the canonical terminal SignedDelta candidate and exact payout.
///
/// This function does not claim settlement. The caller must pass the candidate
/// to a Claims-owned terminal route that atomically executes SignedDelta and
/// Custody, validates both receipts, and returns before Trading commits root.
pub fn plan_fractional_exposure_terminal_candidate_v2(
    terms: FractionalExposureTermsV2<'_>,
    request: FractionalExposureRequestV2,
    terminal: FractionalExposureTerminalInputV2<'_>,
) -> Result<FractionalExposureTerminalCandidateV2> {
    let request = request.bind_terms(terms).map_err(|_| Error::Claims)?;
    if !matches!(
        request.action(),
        FractionalExposureActionV2::TerminalRedeem | FractionalExposureActionV2::TerminalZeroBurn
    ) || terminal.terminal_record_digest != request.input().terminal_digest
    {
        return Err(Error::Claims);
    }
    let input = request.input();
    let admission = terminal.claims.representation;
    let exposure = CompositionExposureBundleV3::decode(
        terminal.claims.composition_exposure_bytes,
        terminal.claims.composition_exposure_admission,
    )
    .map_err(|_| Error::Claims)?;
    check_fractional_exposure_bundle_v2(terms, exposure).map_err(|_| Error::Claims)?;
    if terminal.claims.product_record_digest != input.product_record
        || terminal.fractional_root == [0; 32]
        || terminal.fractional_root == input.owner
        || terminal.claims.owner != terminal.fractional_root
        || terminal.claims.claim_index != input.representation_coordinate
        || terminal.claims.caller_role != CallerRole::Trading
        || admission.market_id() != terms.market()
        || admission.release_set_id() != terms.release_set()
        || admission.result_domain_id() != terms.result_domain()
        || admission.linked_basis_record_digest() != terms.product_basis()
        || admission.semantic_basis_id() != terms.representation_basis()
        || admission.graph_id() != terms.exposure_id()
        || admission.basis_width() != terms.representation_width()
        || admission.token_program() != terms.token_program()
        || terminal.claims.composition_exposure_admission.selected_id != terms.exposure_id()
    {
        return Err(Error::Claims);
    }
    let division =
        divide_exposure_shards_v2(terms, input.representation_coordinate, input.quantity)
            .map_err(|_| Error::Claims)?;
    let request_bytes = request.to_bytes().map_err(|_| Error::Claims)?;
    let request_digest: [u8; 32] = Sha256::digest(request_bytes).into();
    let claims_width = usize::try_from(terms.representation_width()).map_err(|_| Error::Claims)?;
    let product_width = usize::try_from(terms.product_width()).map_err(|_| Error::Claims)?;
    let neutral = SignedDeltaV3::new(
        dclutch_claims_svm::signed_delta_v3::DeltaDirectionV3::Neutral,
        0,
    )
    .map_err(|_| Error::Claims)?;
    let mut product_payout_scratch = vec![0_u64; product_width];
    let mut translation_scratch = vec![0_u64; claims_width];
    let mut claims_payout_scratch = vec![0_u64; claims_width];
    let mut aggregate_delta_scratch = vec![neutral; claims_width];
    let mut packet =
        vec![0_u8; plan_bytes(terms.representation_width(), 1, 1).map_err(|_| Error::Claims)?];
    let mut claims = terminal.claims;
    claims.request_id = request_digest;
    claims.claim_index = input.representation_coordinate;
    claims.quantity = division.whole_claims;
    let collateral_atoms = encode_product_basis_terminal_signed_delta_v3(
        claims,
        &mut product_payout_scratch,
        &mut translation_scratch,
        &mut claims_payout_scratch,
        &mut aggregate_delta_scratch,
        &mut packet,
    )
    .map_err(|_| Error::Claims)?;
    if (request.action() == FractionalExposureActionV2::TerminalRedeem && collateral_atoms == 0)
        || (request.action() == FractionalExposureActionV2::TerminalZeroBurn
            && collateral_atoms != 0)
    {
        return Err(Error::Claims);
    }
    Ok(FractionalExposureTerminalCandidateV2 {
        packet,
        request_digest,
        collateral_atoms,
        division,
    })
}

fn checked_holder_v2<'a>(
    terms: FractionalExposureTermsV2<'_>,
    account: Option<FractionalTokenAccountSnapshotV1<'a>>,
    expected_key: [u8; 32],
    expected_mint: [u8; 32],
    expected_owner: [u8; 32],
    expected_amount: u64,
) -> Result<FractionalTokenAccountSnapshotV1<'a>> {
    let account = account.ok_or(Error::Token)?;
    if account.key.to_bytes() != expected_key
        || account.program_owner.to_bytes() != terms.token_program()
    {
        return Err(Error::Token);
    }
    Token2022BehaviorProfileV2::check_account(
        terms.token_program(),
        account.data,
        expected_mint,
        expected_owner,
        expected_amount,
    )
    .map_err(|_| Error::Token)?;
    Ok(account)
}

fn checked_holder_any_owner_v2<'a>(
    terms: FractionalExposureTermsV2<'_>,
    account: Option<FractionalTokenAccountSnapshotV1<'a>>,
    expected_key: [u8; 32],
    expected_mint: [u8; 32],
    expected_amount: u64,
) -> Result<FractionalTokenAccountSnapshotV1<'a>> {
    let account = account.ok_or(Error::Token)?;
    let parsed = TokenAccount::parse(account.data).map_err(|_| Error::Token)?;
    if parsed.owner == [0; 32] {
        return Err(Error::Token);
    }
    checked_holder_v2(
        terms,
        Some(account),
        expected_key,
        expected_mint,
        parsed.owner,
        expected_amount,
    )
}

/// Rebuild finalized exposure admission from independently authenticated bytes.
pub fn fractional_exposure_record_admission_v2(
    terms: FractionalExposureTermsV2<'_>,
    recomputed_digest: [u8; 32],
    finalized_digest: [u8; 32],
    record_authenticated: bool,
) -> RecordAdmissionV3 {
    RecordAdmissionV3 {
        selected_id: terms.exposure_id(),
        finalized_id: terms.exposure_id(),
        recomputed_digest,
        finalized_digest,
        record_authenticated,
    }
}
