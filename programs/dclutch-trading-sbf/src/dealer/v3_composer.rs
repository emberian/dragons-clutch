//! Atomic scenario-solvent portfolio-fill composition for Dealer V3.
//!
//! This planner joins the canonical Trading-owned obligation pre/poststates,
//! the canonical Claims Position, exact portfolio legs, quote principal, and
//! segregated realized fees.  It derives complete-set split/merge through the
//! sole Dealer scenario kernel and emits only canonical Custody requests.
//! Claims remains the sole mutation authority: the returned delta commitment
//! must be matched by the family-neutral signed-delta Claims V3 packet and its
//! immediate receipt before Trading commits the obligation state last.

use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyReceiptV1, CustodyRequestV1,
    CustodyVaultSeedsV1, OperationV1,
};
use dclutch_dealer_codec::scenario::{
    ClaimsInventoryObservation, DescriptorScenarioInput, ScenarioPlan, plan_descriptor_scenario,
};
use solana_program::{
    hash::{hash, hashv},
    pubkey::Pubkey,
};

use super::v3_obligation::DealerObligationProjectionV3;

/// Maximum distinct Custody transfers in one scenario-solvent fill.
pub const MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3: usize = 4;
/// Domain for the expected family-neutral Claims signed-delta commitment.
pub const DEALER_CLAIMS_DELTA_DOMAIN_V3: &[u8] = b"dclutch:dealer-claims-delta:v3";
/// Per-coordinate chaining domain for the runtime-width delta commitment.
pub const DEALER_CLAIMS_DELTA_ITEM_DOMAIN_V3: &[u8] = b"dclutch:dealer-delta-item:v3";

/// Stable refusal from atomic scenario-fill composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioComposerErrorV3 {
    /// A required identity, owner, PDA, or immutable coordinate differed.
    CoordinateMismatch,
    /// Runtime-width Position, obligation, transfer, or scratch slices differed.
    WidthMismatch,
    /// Obligation pre/poststates did not advance exactly once.
    ObligationTransition,
    /// Quote, fee, token balance, replay, or revision arithmetic failed.
    Arithmetic,
    /// The sole scenario-solvency planner refused the candidate.
    Insolvent,
    /// A canonical Custody request could not be constructed.
    Custody,
    /// A child receipt or exact physical postcondition differed.
    Postcondition,
}

/// Result alias for scenario-fill composition.
pub type ScenarioComposerResultV3<T> = core::result::Result<T, ScenarioComposerErrorV3>;

/// Direction of the exact principal quote leg.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioQuoteDirectionV3 {
    /// Counterparty pays quote principal into Dealer TradingPrincipal.
    CounterpartyPaysDealer,
    /// Dealer TradingPrincipal pays quote principal to the counterparty.
    DealerPaysCounterparty,
}

/// Exact priced quote and cumulative realized-fee leg.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioQuoteLegV3 {
    /// Principal direction.
    pub direction: ScenarioQuoteDirectionV3,
    /// Positive exact quote principal atoms.
    pub principal: u64,
    /// Exact realized fee atoms, transferred separately into FeeVault.
    pub realized_fee: u64,
}

/// Common authenticated coordinates for one scenario fill.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioComposerContextV3 {
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Current Registry-selected Custody program.
    pub custody_program: [u8; 32],
    /// Immutable execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable Realm selecting collateral.
    pub realm: [u8; 32],
    /// Immutable Trading child root and Custody replay context.
    pub child_root: [u8; 32],
    /// Canonical obligation PDA address.
    pub obligation_account: [u8; 32],
    /// Realm-selected collateral Mint.
    pub mint: [u8; 32],
    /// Realm-selected Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Digest of the exact parent Trading request.
    pub parent_request_digest: [u8; 32],
    /// Current Core Market generation.
    pub generation: u64,
    /// Custody replay revision before the first transfer.
    pub custody_replay_revision: u64,
    /// Locked capital floor from the exact selected descriptor.
    pub locked_capital_floor: u64,
}

/// Exact collateral accounts and authenticated pre-balances.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioCollateralFrameV3 {
    /// Canonical TradingPrincipal vault.
    pub principal_vault: [u8; 32],
    /// Present TradingPrincipal balance; the only eligible scalar capital.
    pub principal_balance: u64,
    /// Physically distinct realized FeeVault.
    pub fee_vault: [u8; 32],
    /// Present realized fee balance, excluded from capital.
    pub fee_balance: u64,
    /// Canonical Market HoardPrincipal vault.
    pub hoard_vault: [u8; 32],
    /// Present Hoard principal balance.
    pub hoard_balance: u64,
    /// Counterparty-owned external collateral token account.
    pub counterparty_account: [u8; 32],
    /// Counterparty authority and Claims Position owner.
    pub counterparty_owner: [u8; 32],
    /// Present external token balance.
    pub counterparty_balance: u64,
}

/// Exact portfolio and quote candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioFillInputV3<'a> {
    /// Canonical Dealer Claims Position before the fill.
    pub dealer_position: ClaimsInventoryObservation<'a>,
    /// Optimistic counterparty Claims Position revision.
    pub counterparty_position_revision: u64,
    /// Nonnegative Claims transferred from counterparty to Dealer.
    pub acquired: &'a [u64],
    /// Nonnegative Claims transferred from Dealer to counterparty.
    pub delivered: &'a [u64],
    /// Exact principal/fee leg.
    pub quote: ScenarioQuoteLegV3,
}

/// One exact Custody request and required two-account balances afterward.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioCustodyEffectV3 {
    /// Exact canonical Custody request.
    pub request: CustodyRequestV1,
    /// Required source balance immediately afterward.
    pub source_after: u64,
    /// Required destination balance immediately afterward.
    pub destination_after: u64,
}

/// Exact semantic commitment required from Claims V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioClaimsDeltaExpectationV3 {
    /// Canonical Dealer Claims Position owner.
    pub dealer_owner: [u8; 32],
    /// Exact counterparty Claims Position owner.
    pub counterparty_owner: [u8; 32],
    /// Dealer Position optimistic pre-revision.
    pub dealer_revision_before: u64,
    /// Dealer Position exact post-revision.
    pub dealer_revision_after: u64,
    /// Counterparty Position optimistic pre-revision.
    pub counterparty_revision_before: u64,
    /// Counterparty Position exact post-revision.
    pub counterparty_revision_after: u64,
    /// Claims aggregate optimistic pre-revision.
    pub claims_revision_before: u64,
    /// Claims aggregate exact post-revision.
    pub claims_revision_after: u64,
    /// Runtime Product width.
    pub width: u32,
    /// Minimum complete-set split admitted by the sole scenario kernel.
    pub split: u64,
    /// Maximum complete-set merge admitted by the sole scenario kernel.
    pub merge: u64,
    /// Commitment to unique signed Dealer/counterparty/aggregate deltas.
    pub delta_digest: [u8; 32],
}

/// Complete staged portfolio fill before any CPI or write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScenarioAtomicPlanV3 {
    /// Exact scenario-solvency and split/merge plan.
    pub scenario: ScenarioPlan,
    /// Expected family-neutral Claims V3 semantics.
    pub claims: ScenarioClaimsDeltaExpectationV3,
    /// Ordered Custody requests; inactive capacity is `None`.
    pub custody: [Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    /// Active Custody request count.
    pub custody_count: u8,
    /// Required final TradingPrincipal balance.
    pub principal_after: u64,
    /// Required final FeeVault balance.
    pub fee_after: u64,
    /// Required final HoardPrincipal balance.
    pub hoard_after: u64,
    /// Required final counterparty external balance.
    pub counterparty_after: u64,
    /// Required candidate obligation-state digest committed last.
    pub obligation_digest_after: [u8; 32],
    /// Candidate obligation revision committed last.
    pub obligation_revision_after: u64,
}

/// Preflight the entire scenario-solvent physical fill.
///
/// The current and candidate obligation projections must come from the one
/// Trading-owned PDA.  `post_inventory` and `post_equity` are written only if
/// every identity, solvency, quote, fee, and Custody check succeeds.
#[allow(clippy::too_many_arguments)]
pub fn prepare_scenario_atomic_v3(
    context: ScenarioComposerContextV3,
    frame: ScenarioCollateralFrameV3,
    current_obligation: DealerObligationProjectionV3<'_>,
    candidate_obligation: DealerObligationProjectionV3<'_>,
    candidate_obligation_digest: [u8; 32],
    claims_market_revision: u64,
    input: ScenarioFillInputV3<'_>,
    obligations_before: &mut [u64],
    obligations_after: &mut [u64],
    post_inventory: &mut [u64],
    post_equity: &mut [i128],
) -> ScenarioComposerResultV3<ScenarioAtomicPlanV3> {
    validate_coordinates(
        context,
        frame,
        current_obligation,
        candidate_obligation,
        candidate_obligation_digest,
        input,
    )?;
    let width = usize::try_from(current_obligation.width())
        .map_err(|_| ScenarioComposerErrorV3::WidthMismatch)?;
    for observed in [
        input.acquired.len(),
        input.delivered.len(),
        obligations_before.len(),
        obligations_after.len(),
        post_inventory.len(),
        post_equity.len(),
    ] {
        if observed != width {
            return Err(ScenarioComposerErrorV3::WidthMismatch);
        }
    }
    for (output, value) in obligations_before
        .iter_mut()
        .zip(current_obligation.obligations())
    {
        *output = value;
    }
    for (output, value) in obligations_after
        .iter_mut()
        .zip(candidate_obligation.obligations())
    {
        *output = value;
    }

    let adjusted_principal = match input.quote.direction {
        ScenarioQuoteDirectionV3::CounterpartyPaysDealer => {
            frame.principal_balance.checked_add(input.quote.principal)
        }
        ScenarioQuoteDirectionV3::DealerPaysCounterparty => {
            frame.principal_balance.checked_sub(input.quote.principal)
        }
    }
    .ok_or(ScenarioComposerErrorV3::Arithmetic)?;
    let scenario = plan_descriptor_scenario(
        DescriptorScenarioInput {
            descriptor: current_obligation.descriptor(context.locked_capital_floor),
            position: input.dealer_position,
            expected_position_revision: input.dealer_position.revision,
            present_capital: adjusted_principal,
            obligations_before,
            acquired: input.acquired,
            delivered: input.delivered,
            obligations_after,
        },
        post_inventory,
        post_equity,
    )
    .map_err(|_| ScenarioComposerErrorV3::Insolvent)?;

    let mut balances = ComposerBalancesV3 {
        principal: frame.principal_balance,
        fee: frame.fee_balance,
        hoard: frame.hoard_balance,
        counterparty: frame.counterparty_balance,
    };
    let mut custody = [None; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3];
    let mut count = 0_usize;
    if input.quote.direction == ScenarioQuoteDirectionV3::CounterpartyPaysDealer {
        push_transfer(
            &mut custody,
            &mut count,
            context,
            frame,
            TransferKindV3::CounterpartyToPrincipal,
            input.quote.principal,
            &mut balances,
        )?;
    }
    let fee_kind = match input.quote.direction {
        ScenarioQuoteDirectionV3::CounterpartyPaysDealer => TransferKindV3::CounterpartyToFee,
        ScenarioQuoteDirectionV3::DealerPaysCounterparty => TransferKindV3::PrincipalToFee,
    };
    push_transfer(
        &mut custody,
        &mut count,
        context,
        frame,
        fee_kind,
        input.quote.realized_fee,
        &mut balances,
    )?;
    push_transfer(
        &mut custody,
        &mut count,
        context,
        frame,
        TransferKindV3::PrincipalToHoard,
        scenario.minimum_complete_sets_to_split,
        &mut balances,
    )?;
    push_transfer(
        &mut custody,
        &mut count,
        context,
        frame,
        TransferKindV3::HoardToPrincipal,
        scenario.maximum_complete_sets_to_merge,
        &mut balances,
    )?;
    if input.quote.direction == ScenarioQuoteDirectionV3::DealerPaysCounterparty {
        let counterparty_proceeds = input
            .quote
            .principal
            .checked_sub(input.quote.realized_fee)
            .ok_or(ScenarioComposerErrorV3::Arithmetic)?;
        push_transfer(
            &mut custody,
            &mut count,
            context,
            frame,
            TransferKindV3::PrincipalToCounterparty,
            counterparty_proceeds,
            &mut balances,
        )?;
    }
    if balances.principal != scenario.capital_after {
        return Err(ScenarioComposerErrorV3::Arithmetic);
    }

    let claims_revision_after = claims_market_revision
        .checked_add(1)
        .ok_or(ScenarioComposerErrorV3::Arithmetic)?;
    let counterparty_revision_after = input
        .counterparty_position_revision
        .checked_add(1)
        .ok_or(ScenarioComposerErrorV3::Arithmetic)?;
    let claims = ScenarioClaimsDeltaExpectationV3 {
        dealer_owner: input.dealer_position.position_owner,
        counterparty_owner: frame.counterparty_owner,
        dealer_revision_before: input.dealer_position.revision,
        dealer_revision_after: scenario.position_revision_after,
        counterparty_revision_before: input.counterparty_position_revision,
        counterparty_revision_after,
        claims_revision_before: claims_market_revision,
        claims_revision_after,
        width: current_obligation.width(),
        split: scenario.minimum_complete_sets_to_split,
        merge: scenario.maximum_complete_sets_to_merge,
        delta_digest: claims_delta_digest(
            input,
            scenario.minimum_complete_sets_to_split,
            scenario.maximum_complete_sets_to_merge,
        )?,
    };
    Ok(ScenarioAtomicPlanV3 {
        scenario,
        claims,
        custody,
        custody_count: u8::try_from(count).map_err(|_| ScenarioComposerErrorV3::Arithmetic)?,
        principal_after: balances.principal,
        fee_after: balances.fee,
        hoard_after: balances.hoard,
        counterparty_after: balances.counterparty,
        obligation_digest_after: candidate_obligation_digest,
        obligation_revision_after: candidate_obligation.revision(),
    })
}

/// Verify one immediate Custody acknowledgement against an exact plan effect.
pub fn verify_scenario_custody_receipt_v3(
    effect: ScenarioCustodyEffectV3,
    receipt_bytes: &[u8],
    poststate_commitment: [u8; 32],
) -> ScenarioComposerResultV3<()> {
    let request_bytes = effect
        .request
        .to_bytes()
        .map_err(|_| ScenarioComposerErrorV3::Custody)?;
    let receipt =
        CustodyReceiptV1::decode(receipt_bytes).map_err(|_| ScenarioComposerErrorV3::Custody)?;
    receipt
        .verify_for(
            effect.request,
            hash(&request_bytes).to_bytes(),
            poststate_commitment,
        )
        .map_err(|_| ScenarioComposerErrorV3::Custody)?;
    if receipt.evidence.source_after != effect.source_after
        || receipt.evidence.destination_after != effect.destination_after
    {
        return Err(ScenarioComposerErrorV3::Postcondition);
    }
    Ok(())
}

/// Verify exact write-last state after all Claims/Custody receipts passed.
///
/// `validated_claims_delta_digest` must come from the current Claims V3
/// producer receipt validator, never from the request or a static client.
#[allow(clippy::too_many_arguments)]
pub fn verify_scenario_postconditions_v3(
    plan: ScenarioAtomicPlanV3,
    validated_claims_delta_digest: [u8; 32],
    observed_principal: u64,
    observed_fee: u64,
    observed_hoard: u64,
    observed_counterparty: u64,
    observed_obligation: &[u8],
    expected_post_inventory: &[u64],
    observed_post_inventory: &[u64],
) -> ScenarioComposerResultV3<()> {
    let obligation = DealerObligationProjectionV3::decode(observed_obligation)
        .map_err(|_| ScenarioComposerErrorV3::Postcondition)?;
    if validated_claims_delta_digest != plan.claims.delta_digest
        || observed_principal != plan.principal_after
        || observed_fee != plan.fee_after
        || observed_hoard != plan.hoard_after
        || observed_counterparty != plan.counterparty_after
        || hash(observed_obligation).to_bytes() != plan.obligation_digest_after
        || obligation.revision() != plan.obligation_revision_after
        || expected_post_inventory != observed_post_inventory
        || observed_post_inventory.len()
            != usize::try_from(plan.claims.width)
                .map_err(|_| ScenarioComposerErrorV3::WidthMismatch)?
    {
        return Err(ScenarioComposerErrorV3::Postcondition);
    }
    Ok(())
}

fn validate_coordinates(
    context: ScenarioComposerContextV3,
    frame: ScenarioCollateralFrameV3,
    current: DealerObligationProjectionV3<'_>,
    candidate: DealerObligationProjectionV3<'_>,
    candidate_digest: [u8; 32],
    input: ScenarioFillInputV3<'_>,
) -> ScenarioComposerResultV3<()> {
    for identity in [
        context.trading_program,
        context.custody_program,
        context.release_set,
        context.market,
        context.realm,
        context.child_root,
        context.obligation_account,
        context.mint,
        context.token_program,
        context.parent_request_digest,
        frame.principal_vault,
        frame.fee_vault,
        frame.hoard_vault,
        frame.counterparty_account,
        frame.counterparty_owner,
        candidate_digest,
    ] {
        if identity == [0; 32] {
            return Err(ScenarioComposerErrorV3::CoordinateMismatch);
        }
    }
    let current_descriptor = current.descriptor(context.locked_capital_floor);
    let candidate_descriptor = candidate.descriptor(context.locked_capital_floor);
    let principal = custody_vault(context, context.child_root, CompartmentV1::TradingPrincipal);
    let fee = custody_vault(context, context.child_root, CompartmentV1::FeeVault);
    let hoard = custody_vault(context, context.market, CompartmentV1::HoardPrincipal);
    if current_descriptor != candidate_descriptor
        || current.child_root() != context.child_root
        || current_descriptor.market_id != context.market
        || current.position_owner() != input.dealer_position.position_owner
        || current.width() != candidate.width()
        || usize::try_from(current.width()).ok() != Some(input.dealer_position.inventory.len())
        || current.lp_principal() != candidate.lp_principal()
        || current.revision().checked_add(1) != Some(candidate.revision())
        || candidate.state_digest() != candidate_digest
        || input.dealer_position.market_id != context.market
        || input.dealer_position.product_id != current_descriptor.product_id
        || input.dealer_position.liability_basis_id != current_descriptor.liability_basis_id
        || frame.principal_vault != principal
        || frame.fee_vault != fee
        || frame.hoard_vault != hoard
        || frame.principal_vault == frame.fee_vault
        || frame.principal_vault == frame.hoard_vault
        || frame.counterparty_account == frame.principal_vault
        || input.quote.principal == 0
    {
        return Err(ScenarioComposerErrorV3::CoordinateMismatch);
    }
    Ok(())
}

fn custody_vault(
    context: ScenarioComposerContextV3,
    vault_context: [u8; 32],
    compartment: CompartmentV1,
) -> [u8; 32] {
    Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            context.market,
            context.release_set,
            vault_context,
            compartment,
        )
        .as_slices(),
        &Pubkey::new_from_array(context.custody_program),
    )
    .0
    .to_bytes()
}

#[derive(Clone, Copy)]
enum TransferKindV3 {
    CounterpartyToPrincipal,
    PrincipalToCounterparty,
    CounterpartyToFee,
    PrincipalToFee,
    PrincipalToHoard,
    HoardToPrincipal,
}

#[derive(Clone, Copy)]
struct ComposerBalancesV3 {
    principal: u64,
    fee: u64,
    hoard: u64,
    counterparty: u64,
}

#[allow(clippy::too_many_arguments)]
fn push_transfer(
    output: &mut [Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    count: &mut usize,
    context: ScenarioComposerContextV3,
    frame: ScenarioCollateralFrameV3,
    kind: TransferKindV3,
    amount: u64,
    balances: &mut ComposerBalancesV3,
) -> ScenarioComposerResultV3<()> {
    if amount == 0 {
        return Ok(());
    }
    if *count >= output.len() {
        return Err(ScenarioComposerErrorV3::Arithmetic);
    }
    let (
        source,
        destination,
        source_compartment,
        destination_compartment,
        source_owner,
        destination_owner,
        source_context,
        destination_context,
        source_before,
        destination_before,
    ) = match kind {
        TransferKindV3::CounterpartyToPrincipal => (
            frame.counterparty_account,
            frame.principal_vault,
            CompartmentV1::External,
            CompartmentV1::TradingPrincipal,
            frame.counterparty_owner,
            [0; 32],
            [0; 32],
            context.child_root,
            balances.counterparty,
            balances.principal,
        ),
        TransferKindV3::PrincipalToCounterparty => (
            frame.principal_vault,
            frame.counterparty_account,
            CompartmentV1::TradingPrincipal,
            CompartmentV1::External,
            [0; 32],
            frame.counterparty_owner,
            context.child_root,
            [0; 32],
            balances.principal,
            balances.counterparty,
        ),
        TransferKindV3::CounterpartyToFee => (
            frame.counterparty_account,
            frame.fee_vault,
            CompartmentV1::External,
            CompartmentV1::FeeVault,
            frame.counterparty_owner,
            [0; 32],
            [0; 32],
            context.child_root,
            balances.counterparty,
            balances.fee,
        ),
        TransferKindV3::PrincipalToFee => (
            frame.principal_vault,
            frame.fee_vault,
            CompartmentV1::TradingPrincipal,
            CompartmentV1::FeeVault,
            [0; 32],
            [0; 32],
            context.child_root,
            context.child_root,
            balances.principal,
            balances.fee,
        ),
        TransferKindV3::PrincipalToHoard => (
            frame.principal_vault,
            frame.hoard_vault,
            CompartmentV1::TradingPrincipal,
            CompartmentV1::HoardPrincipal,
            [0; 32],
            [0; 32],
            context.child_root,
            context.market,
            balances.principal,
            balances.hoard,
        ),
        TransferKindV3::HoardToPrincipal => (
            frame.hoard_vault,
            frame.principal_vault,
            CompartmentV1::HoardPrincipal,
            CompartmentV1::TradingPrincipal,
            [0; 32],
            [0; 32],
            context.market,
            context.child_root,
            balances.hoard,
            balances.principal,
        ),
    };
    let source_after = source_before
        .checked_sub(amount)
        .ok_or(ScenarioComposerErrorV3::Arithmetic)?;
    let destination_after = destination_before
        .checked_add(amount)
        .ok_or(ScenarioComposerErrorV3::Arithmetic)?;
    let expected_revision = context
        .custody_replay_revision
        .checked_add(u64::try_from(*count).map_err(|_| ScenarioComposerErrorV3::Arithmetic)?)
        .ok_or(ScenarioComposerErrorV3::Arithmetic)?;
    let request = CustodyRequestV1 {
        operation: OperationV1::Transfer,
        caller_role: CallerRoleV1::Trading,
        source_compartment,
        destination_compartment,
        release_set: context.release_set,
        market: context.market,
        realm: context.realm,
        context: context.child_root,
        caller_program: context.trading_program,
        semantic: ContextV1 {
            candidate: context.obligation_account,
            source_owner,
            destination_owner,
            order: frame.counterparty_owner,
            parent_request_digest: context.parent_request_digest,
            order_nonce: context.custody_replay_revision,
            generation: context.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: u16::try_from(*count)
                .map_err(|_| ScenarioComposerErrorV3::Arithmetic)?,
        },
        source,
        destination,
        source_vault_context: source_context,
        destination_vault_context: destination_context,
        mint: context.mint,
        token_program: context.token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or(ScenarioComposerErrorV3::Arithmetic)?,
        amount,
        rent_lamports: 0,
    };
    request
        .validate()
        .map_err(|_| ScenarioComposerErrorV3::Custody)?;
    output[*count] = Some(ScenarioCustodyEffectV3 {
        request,
        source_after,
        destination_after,
    });
    match kind {
        TransferKindV3::CounterpartyToPrincipal => {
            balances.counterparty = source_after;
            balances.principal = destination_after;
        }
        TransferKindV3::PrincipalToCounterparty => {
            balances.principal = source_after;
            balances.counterparty = destination_after;
        }
        TransferKindV3::CounterpartyToFee => {
            balances.counterparty = source_after;
            balances.fee = destination_after;
        }
        TransferKindV3::PrincipalToFee => {
            balances.principal = source_after;
            balances.fee = destination_after;
        }
        TransferKindV3::PrincipalToHoard => {
            balances.principal = source_after;
            balances.hoard = destination_after;
        }
        TransferKindV3::HoardToPrincipal => {
            balances.hoard = source_after;
            balances.principal = destination_after;
        }
    }
    *count = count
        .checked_add(1)
        .ok_or(ScenarioComposerErrorV3::Arithmetic)?;
    Ok(())
}

fn claims_delta_digest(
    input: ScenarioFillInputV3<'_>,
    split: u64,
    merge: u64,
) -> ScenarioComposerResultV3<[u8; 32]> {
    if input.acquired.len() != input.delivered.len() {
        return Err(ScenarioComposerErrorV3::WidthMismatch);
    }
    let width =
        u32::try_from(input.acquired.len()).map_err(|_| ScenarioComposerErrorV3::WidthMismatch)?;
    let counterparty_revision = input.counterparty_position_revision.to_le_bytes();
    let split_bytes = split.to_le_bytes();
    let merge_bytes = merge.to_le_bytes();
    let width_bytes = width.to_le_bytes();
    let mut digest = hashv(&[
        DEALER_CLAIMS_DELTA_DOMAIN_V3,
        &input.dealer_position.position_owner,
        &counterparty_revision,
        &split_bytes,
        &merge_bytes,
        &width_bytes,
    ])
    .to_bytes();
    for (acquired, delivered) in input.acquired.iter().zip(input.delivered.iter()) {
        let acquired = acquired.to_le_bytes();
        let delivered = delivered.to_le_bytes();
        digest = hashv(&[
            DEALER_CLAIMS_DELTA_ITEM_DOMAIN_V3,
            &digest,
            &acquired,
            &delivered,
        ])
        .to_bytes();
    }
    Ok(digest)
}
