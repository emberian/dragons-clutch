//! Scenario-solvent, custody-backed multi-LP capital for Dealer V3.
//!
//! Each liquidity provider owns one Trading PDA whose `principal_shares` are
//! exact collateral atoms.  Adding one share moves one present collateral atom
//! into TradingPrincipal and adds one par obligation in every terminal
//! scenario.  Removing one share performs the inverse only when both the
//! incoming and candidate state satisfy the descriptor's locked floor.  Fees,
//! future order flow, and Market Hoard principal are structurally absent.

use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyReceiptV1, CustodyRequestV1,
    CustodyVaultSeedsV1, OperationV1,
};
use dclutch_dealer_codec::scenario::{
    ClaimsInventoryObservation, DescriptorSolvencyInput, ScenarioSolvencyReport,
    assess_descriptor_solvency,
};
use solana_program::{hash::hash, pubkey::Pubkey};

use super::v3_obligation::{
    DealerObligationProjectionV3, LpPrincipalDeltaV3, ObligationErrorV3,
    stage_lp_principal_delta_v3,
};

/// PDA domain for one LP position beneath a canonical Dealer child root.
pub const DEALER_LP_POSITION_PDA_DOMAIN_V3: &[u8] = b"dclutch:dealer-lp-position:v3";
/// Exact fixed LP position bytes.
pub const DEALER_LP_POSITION_BYTES_V3: usize = 256;
/// Exact LP position wire magic.
pub const DEALER_LP_POSITION_MAGIC_V3: [u8; 8] = *b"DCLDLP03";
/// Current LP position wire version.
pub const DEALER_LP_POSITION_VERSION_V3: u16 = 1;

const _: () = assert!(DEALER_LP_POSITION_PDA_DOMAIN_V3.len() <= 32);

/// Stable refusal from multi-LP admission or acknowledgement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiLpErrorV3 {
    /// A required identity was zero or bytes were noncanonical.
    InvalidState,
    /// LP Position key, owner, child root, or signer identity differed.
    PositionMismatch,
    /// Canonical obligation state differed or could not stage the exact delta.
    Obligation,
    /// Runtime Product widths differed.
    WidthMismatch,
    /// Incoming or candidate scenario solvency refused.
    Insolvent,
    /// Exact collateral, share, balance, or revision arithmetic failed.
    Arithmetic,
    /// Custody endpoint, request, receipt, or balance postcondition refused.
    Custody,
    /// A poststate digest or revision differed from the admitted candidate.
    Postcondition,
}

impl From<ObligationErrorV3> for MultiLpErrorV3 {
    fn from(_: ObligationErrorV3) -> Self {
        Self::Obligation
    }
}

/// Result alias for multi-LP physical planning.
pub type MultiLpResultV3<T> = core::result::Result<T, MultiLpErrorV3>;

/// LP capital operation selected by the exact request profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiLpActionV3 {
    /// Move present external collateral into TradingPrincipal.
    Add,
    /// Return present TradingPrincipal collateral to its LP owner.
    Remove,
}

/// Canonical decoded LP Position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLpPositionV3 {
    /// Current optimistic revision.
    pub revision: u64,
    /// Immutable execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Immutable Trading child root.
    pub child_root: [u8; 32],
    /// LP authority and external collateral owner.
    pub lp_owner: [u8; 32],
    /// Immutable recipient of prepaid account rent on close.
    pub rent_refund: [u8; 32],
    /// Canonical Dealer obligation PDA joined by this Position.
    pub obligation_account: [u8; 32],
    /// Exact outstanding par principal shares.
    pub principal_shares: u64,
    /// Core Market generation.
    pub generation: u64,
}

impl DealerLpPositionV3 {
    /// Hostile-decode one exact fixed LP Position.
    pub fn decode(bytes: &[u8]) -> MultiLpResultV3<Self> {
        if bytes.len() != DEALER_LP_POSITION_BYTES_V3
            || bytes.get(..8) != Some(DEALER_LP_POSITION_MAGIC_V3.as_slice())
            || read_u16(bytes, 8)? != DEALER_LP_POSITION_VERSION_V3
            || bytes.get(10..16).is_none_or(|reserved| reserved != [0; 6])
            || bytes
                .get(232..)
                .is_none_or(|reserved| reserved.iter().any(|byte| *byte != 0))
        {
            return Err(MultiLpErrorV3::InvalidState);
        }
        let value = Self {
            revision: read_u64(bytes, 16)?,
            release_set: read_identity(bytes, 24)?,
            market: read_identity(bytes, 56)?,
            child_root: read_identity(bytes, 88)?,
            lp_owner: read_identity(bytes, 120)?,
            rent_refund: read_identity(bytes, 152)?,
            obligation_account: read_identity(bytes, 184)?,
            principal_shares: read_u64(bytes, 216)?,
            generation: read_u64(bytes, 224)?,
        };
        if value.revision == 0 {
            return Err(MultiLpErrorV3::InvalidState);
        }
        Ok(value)
    }

    /// Encode the exact canonical state.
    pub fn encode_into(self, output: &mut [u8]) -> MultiLpResultV3<()> {
        if output.len() != DEALER_LP_POSITION_BYTES_V3 || self.revision == 0 {
            return Err(MultiLpErrorV3::InvalidState);
        }
        for identity in [
            self.release_set,
            self.market,
            self.child_root,
            self.lp_owner,
            self.rent_refund,
            self.obligation_account,
        ] {
            if identity == [0; 32] {
                return Err(MultiLpErrorV3::InvalidState);
            }
        }
        output.fill(0);
        output[..8].copy_from_slice(&DEALER_LP_POSITION_MAGIC_V3);
        output[8..10].copy_from_slice(&DEALER_LP_POSITION_VERSION_V3.to_le_bytes());
        write_u64(output, 16, self.revision)?;
        for (offset, value) in [
            (24, self.release_set),
            (56, self.market),
            (88, self.child_root),
            (120, self.lp_owner),
            (152, self.rent_refund),
            (184, self.obligation_account),
        ] {
            output[offset..offset + 32].copy_from_slice(&value);
        }
        write_u64(output, 216, self.principal_shares)?;
        write_u64(output, 224, self.generation)?;
        Self::decode(output).map(|_| ())
    }
}

/// Hostile LP Position account observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLpAccountObservationV3<'a> {
    /// Observed account address.
    pub address: [u8; 32],
    /// Observed account owner.
    pub owner: [u8; 32],
    /// Exact current data.
    pub data: &'a [u8],
}

/// Prepared creation of one vacant prepaid LP Position PDA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLpOpenPlanV3 {
    /// Exact vacant PDA to allocate and assign to Trading.
    pub position: [u8; 32],
    /// Exact payer of already-present rent lamports.
    pub payer: [u8; 32],
    /// Immutable refund recipient persisted in the new Position.
    pub rent_refund: [u8; 32],
    /// Exact rent-exempt lamports required for the fixed Position width.
    pub rent_lamports: u64,
    /// Initial canonical Position bytes committed after System CPI.
    pub initial_state: [u8; DEALER_LP_POSITION_BYTES_V3],
    /// Digest of the exact initial state.
    pub initial_state_digest: [u8; 32],
}

/// Prepare allocation/assignment of one wholly vacant prepaid LP Position.
///
/// The common Trading outer performs System allocate/assign with the PDA,
/// verifies owner/lamports/data length, and commits `initial_state` last.
pub fn prepare_lp_open_v3(
    context: MultiLpContextV3,
    lp_owner: [u8; 32],
    payer: [u8; 32],
    rent_refund: [u8; 32],
    vacant_address: [u8; 32],
    vacant_owner: [u8; 32],
    vacant_lamports: u64,
    vacant_data: &[u8],
    required_rent_lamports: u64,
) -> MultiLpResultV3<DealerLpOpenPlanV3> {
    for identity in [
        context.trading_program,
        context.release_set,
        context.market,
        context.child_root,
        context.obligation_account,
        lp_owner,
        payer,
        rent_refund,
    ] {
        if identity == [0; 32] {
            return Err(MultiLpErrorV3::InvalidState);
        }
    }
    let expected_address = Pubkey::find_program_address(
        &[
            DEALER_LP_POSITION_PDA_DOMAIN_V3,
            &context.child_root,
            &lp_owner,
        ],
        &Pubkey::new_from_array(context.trading_program),
    )
    .0
    .to_bytes();
    if vacant_address != expected_address
        || vacant_owner != solana_system_interface::program::ID.to_bytes()
        || vacant_lamports != required_rent_lamports
        || required_rent_lamports == 0
        || !vacant_data.is_empty()
    {
        return Err(MultiLpErrorV3::PositionMismatch);
    }
    let mut initial_state = [0; DEALER_LP_POSITION_BYTES_V3];
    DealerLpPositionV3 {
        revision: 1,
        release_set: context.release_set,
        market: context.market,
        child_root: context.child_root,
        lp_owner,
        rent_refund,
        obligation_account: context.obligation_account,
        principal_shares: 0,
        generation: context.generation,
    }
    .encode_into(&mut initial_state)?;
    Ok(DealerLpOpenPlanV3 {
        position: vacant_address,
        payer,
        rent_refund,
        rent_lamports: required_rent_lamports,
        initial_state_digest: hash(&initial_state).to_bytes(),
        initial_state,
    })
}

/// Prepared quiescent closure of one zero-share LP Position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLpClosePlanV3 {
    /// Exact canonical Position PDA.
    pub position: [u8; 32],
    /// Persisted immutable rent beneficiary.
    pub rent_refund: [u8; 32],
    /// Exact lamports returned from the Position.
    pub rent_lamports: u64,
    /// Digest of the exact zero-share prestate.
    pub prestate_digest: [u8; 32],
    /// Final optimistic revision being retired.
    pub terminal_revision: u64,
}

/// Prepare closure only after all principal shares have exited.
pub fn prepare_lp_close_v3(
    context: MultiLpContextV3,
    observation: DealerLpAccountObservationV3<'_>,
    lp_owner: [u8; 32],
    expected_revision: u64,
    expected_digest: [u8; 32],
    position_lamports: u64,
) -> MultiLpResultV3<DealerLpClosePlanV3> {
    let lp = authenticate_lp_position(context, lp_owner, observation)?;
    if expected_digest == [0; 32]
        || hash(observation.data).to_bytes() != expected_digest
        || lp.revision != expected_revision
        || lp.principal_shares != 0
        || position_lamports == 0
    {
        return Err(MultiLpErrorV3::PositionMismatch);
    }
    Ok(DealerLpClosePlanV3 {
        position: observation.address,
        rent_refund: lp.rent_refund,
        rent_lamports: position_lamports,
        prestate_digest: expected_digest,
        terminal_revision: lp.revision,
    })
}

/// Exact common coordinates for one multi-LP operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiLpContextV3 {
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
    /// Canonical Trading child root and Custody replay context.
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
    /// Custody replay revision before this operation.
    pub custody_replay_revision: u64,
    /// Locked capital floor from the selected immutable Dealer descriptor.
    pub locked_capital_floor: u64,
}

/// Exact collateral endpoints and authenticated pre-balances.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiLpCollateralFrameV3 {
    /// LP-owned external token account.
    pub lp_external_account: [u8; 32],
    /// Exact LP owner, required to sign the outer request.
    pub lp_owner: [u8; 32],
    /// External balance before the operation.
    pub lp_external_balance: u64,
    /// Canonical TradingPrincipal vault.
    pub principal_vault: [u8; 32],
    /// TradingPrincipal balance before the operation.
    pub principal_balance: u64,
}

/// User economic intent after chain-derived optimistic coordinates are fixed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiLpIntentV3 {
    /// Add or remove present par principal.
    pub action: MultiLpActionV3,
    /// Positive exact collateral atoms and principal shares.
    pub amount: u64,
    /// Expected LP Position revision.
    pub expected_lp_revision: u64,
    /// Digest of exact LP Position prestate bytes.
    pub expected_lp_digest: [u8; 32],
}

/// Exact Custody transfer and post-balance evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiLpCustodyEffectV3 {
    /// Canonical Custody request.
    pub request: CustodyRequestV1,
    /// Required external account balance after execution.
    pub external_after: u64,
    /// Required TradingPrincipal balance after execution.
    pub principal_after: u64,
}

/// Complete preflighted multi-LP candidate; Trading-owned writes commit last.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiLpPlanV3 {
    /// Selected action.
    pub action: MultiLpActionV3,
    /// Exact LP owner.
    pub lp_owner: [u8; 32],
    /// Exact principal/share quantity.
    pub amount: u64,
    /// Incoming scenario-solvency report.
    pub solvency_before: ScenarioSolvencyReport,
    /// Candidate scenario-solvency report.
    pub solvency_after: ScenarioSolvencyReport,
    /// Exact physical Custody effect.
    pub custody: MultiLpCustodyEffectV3,
    /// Required post-obligation-state digest.
    pub obligation_digest_after: [u8; 32],
    /// Required post-LP-state digest.
    pub lp_digest_after: [u8; 32],
    /// Required next obligation revision.
    pub obligation_revision_after: u64,
    /// Required next LP Position revision.
    pub lp_revision_after: u64,
}

/// Plan one multi-LP capital change without performing CPI or writes.
///
/// Both poststate buffers remain byte-for-byte unchanged on every refusal.
/// The obligation and equity slices are explicitly caller-owned scratch and
/// must not be treated as authoritative state.
#[allow(clippy::too_many_arguments)]
pub fn prepare_multi_lp_v3(
    context: MultiLpContextV3,
    collateral: MultiLpCollateralFrameV3,
    lp_account: DealerLpAccountObservationV3<'_>,
    obligation: DealerObligationProjectionV3<'_>,
    claims_position: ClaimsInventoryObservation<'_>,
    intent: MultiLpIntentV3,
    obligation_before_scratch: &mut [u64],
    obligation_after_scratch: &mut [u64],
    equity_before: &mut [i128],
    equity_after: &mut [i128],
    post_obligation: &mut [u8],
    post_lp: &mut [u8],
) -> MultiLpResultV3<MultiLpPlanV3> {
    validate_context(context, collateral, obligation, claims_position)?;
    if intent.amount == 0
        || intent.expected_lp_digest == [0; 32]
        || hash(lp_account.data).to_bytes() != intent.expected_lp_digest
        || collateral.lp_owner == [0; 32]
    {
        return Err(MultiLpErrorV3::InvalidState);
    }
    let lp = authenticate_lp_position(context, collateral.lp_owner, lp_account)?;
    if lp.revision != intent.expected_lp_revision {
        return Err(MultiLpErrorV3::PositionMismatch);
    }
    let width = usize::try_from(obligation.width()).map_err(|_| MultiLpErrorV3::WidthMismatch)?;
    for observed in [
        claims_position.inventory.len(),
        obligation_before_scratch.len(),
        obligation_after_scratch.len(),
        equity_before.len(),
        equity_after.len(),
    ] {
        if observed != width {
            return Err(MultiLpErrorV3::WidthMismatch);
        }
    }
    if post_lp.len() != DEALER_LP_POSITION_BYTES_V3 {
        return Err(MultiLpErrorV3::InvalidState);
    }
    let expected_obligation_bytes = super::v3_obligation::DEALER_OBLIGATION_HEADER_BYTES_V3
        .checked_add(width.checked_mul(8).ok_or(MultiLpErrorV3::Arithmetic)?)
        .ok_or(MultiLpErrorV3::Arithmetic)?;
    if post_obligation.len() != expected_obligation_bytes {
        return Err(MultiLpErrorV3::InvalidState);
    }

    let next_lp_shares = match intent.action {
        MultiLpActionV3::Add => lp.principal_shares.checked_add(intent.amount),
        MultiLpActionV3::Remove => lp.principal_shares.checked_sub(intent.amount),
    }
    .ok_or(MultiLpErrorV3::Arithmetic)?;
    let principal_after = match intent.action {
        MultiLpActionV3::Add => collateral.principal_balance.checked_add(intent.amount),
        MultiLpActionV3::Remove => collateral.principal_balance.checked_sub(intent.amount),
    }
    .ok_or(MultiLpErrorV3::Arithmetic)?;
    let external_after = match intent.action {
        MultiLpActionV3::Add => collateral.lp_external_balance.checked_sub(intent.amount),
        MultiLpActionV3::Remove => collateral.lp_external_balance.checked_add(intent.amount),
    }
    .ok_or(MultiLpErrorV3::Arithmetic)?;

    for (output, value) in obligation_before_scratch
        .iter_mut()
        .zip(obligation.obligations())
    {
        *output = value;
    }
    let descriptor = obligation.descriptor(context.locked_capital_floor);
    let solvency_before = assess_descriptor_solvency(
        DescriptorSolvencyInput {
            descriptor,
            position: claims_position,
            expected_position_revision: claims_position.revision,
            present_capital: collateral.principal_balance,
            obligations: obligation_before_scratch,
        },
        equity_before,
    )
    .map_err(|_| MultiLpErrorV3::Insolvent)?;

    let delta = match intent.action {
        MultiLpActionV3::Add => LpPrincipalDeltaV3::Add(intent.amount),
        MultiLpActionV3::Remove => LpPrincipalDeltaV3::Remove(intent.amount),
    };
    for (index, output) in obligation_after_scratch.iter_mut().enumerate() {
        let current = obligation
            .obligation(u32::try_from(index).map_err(|_| MultiLpErrorV3::WidthMismatch)?)?;
        *output = match delta {
            LpPrincipalDeltaV3::Add(amount) => current.checked_add(amount),
            LpPrincipalDeltaV3::Remove(amount) => current.checked_sub(amount),
        }
        .ok_or(MultiLpErrorV3::Arithmetic)?;
    }
    let solvency_after = assess_descriptor_solvency(
        DescriptorSolvencyInput {
            descriptor,
            position: claims_position,
            expected_position_revision: claims_position.revision,
            present_capital: principal_after,
            obligations: obligation_after_scratch,
        },
        equity_after,
    )
    .map_err(|_| MultiLpErrorV3::Insolvent)?;

    let next_lp = DealerLpPositionV3 {
        revision: lp
            .revision
            .checked_add(1)
            .ok_or(MultiLpErrorV3::Arithmetic)?,
        principal_shares: next_lp_shares,
        ..lp
    };
    let mut staged_lp = [0; DEALER_LP_POSITION_BYTES_V3];
    next_lp.encode_into(&mut staged_lp)?;
    let custody = prepare_custody_transfer(
        context,
        collateral,
        intent.action,
        intent.amount,
        external_after,
        principal_after,
    )?;

    stage_lp_principal_delta_v3(obligation, delta, post_obligation)?;
    let staged_projection = DealerObligationProjectionV3::decode(post_obligation)?;
    post_lp.copy_from_slice(&staged_lp);
    Ok(MultiLpPlanV3 {
        action: intent.action,
        lp_owner: collateral.lp_owner,
        amount: intent.amount,
        solvency_before,
        solvency_after,
        custody,
        obligation_digest_after: hash(post_obligation).to_bytes(),
        lp_digest_after: hash(&staged_lp).to_bytes(),
        obligation_revision_after: staged_projection.revision(),
        lp_revision_after: next_lp.revision,
    })
}

/// Verify the immediate Custody receipt and all write-last postconditions.
#[allow(clippy::too_many_arguments)]
pub fn verify_multi_lp_postconditions_v3(
    plan: MultiLpPlanV3,
    custody_receipt: &[u8],
    custody_poststate_commitment: [u8; 32],
    observed_external_balance: u64,
    observed_principal_balance: u64,
    observed_obligation: &[u8],
    observed_lp: &[u8],
) -> MultiLpResultV3<()> {
    let request_bytes = plan
        .custody
        .request
        .to_bytes()
        .map_err(|_| MultiLpErrorV3::Custody)?;
    let receipt = CustodyReceiptV1::decode(custody_receipt).map_err(|_| MultiLpErrorV3::Custody)?;
    receipt
        .verify_for(
            plan.custody.request,
            hash(&request_bytes).to_bytes(),
            custody_poststate_commitment,
        )
        .map_err(|_| MultiLpErrorV3::Custody)?;
    if receipt.evidence.source_after
        != match plan.action {
            MultiLpActionV3::Add => plan.custody.external_after,
            MultiLpActionV3::Remove => plan.custody.principal_after,
        }
        || receipt.evidence.destination_after
            != match plan.action {
                MultiLpActionV3::Add => plan.custody.principal_after,
                MultiLpActionV3::Remove => plan.custody.external_after,
            }
        || observed_external_balance != plan.custody.external_after
        || observed_principal_balance != plan.custody.principal_after
        || hash(observed_obligation).to_bytes() != plan.obligation_digest_after
        || hash(observed_lp).to_bytes() != plan.lp_digest_after
        || DealerObligationProjectionV3::decode(observed_obligation)?.revision()
            != plan.obligation_revision_after
        || DealerLpPositionV3::decode(observed_lp)?.revision != plan.lp_revision_after
    {
        return Err(MultiLpErrorV3::Postcondition);
    }
    Ok(())
}

fn validate_context(
    context: MultiLpContextV3,
    collateral: MultiLpCollateralFrameV3,
    obligation: DealerObligationProjectionV3<'_>,
    claims: ClaimsInventoryObservation<'_>,
) -> MultiLpResultV3<()> {
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
        collateral.lp_external_account,
        collateral.lp_owner,
        collateral.principal_vault,
    ] {
        if identity == [0; 32] {
            return Err(MultiLpErrorV3::InvalidState);
        }
    }
    let expected_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            context.market,
            context.release_set,
            context.child_root,
            CompartmentV1::TradingPrincipal,
        )
        .as_slices(),
        &Pubkey::new_from_array(context.custody_program),
    )
    .0
    .to_bytes();
    let expected_obligation = Pubkey::find_program_address(
        &[
            super::v3_obligation::DEALER_OBLIGATION_PDA_DOMAIN_V3,
            &context.child_root,
        ],
        &Pubkey::new_from_array(context.trading_program),
    )
    .0
    .to_bytes();
    if context.obligation_account != expected_obligation
        || collateral.principal_vault != expected_vault
        || collateral.principal_vault == collateral.lp_external_account
        || obligation.child_root() != context.child_root
        || obligation.position_owner() != claims.position_owner
        || usize::try_from(obligation.width()).ok() != Some(claims.inventory.len())
        || claims.market_id != context.market
    {
        return Err(MultiLpErrorV3::PositionMismatch);
    }
    Ok(())
}

fn authenticate_lp_position(
    context: MultiLpContextV3,
    lp_owner: [u8; 32],
    observation: DealerLpAccountObservationV3<'_>,
) -> MultiLpResultV3<DealerLpPositionV3> {
    let expected_address = Pubkey::find_program_address(
        &[
            DEALER_LP_POSITION_PDA_DOMAIN_V3,
            &context.child_root,
            &lp_owner,
        ],
        &Pubkey::new_from_array(context.trading_program),
    )
    .0
    .to_bytes();
    if observation.address != expected_address || observation.owner != context.trading_program {
        return Err(MultiLpErrorV3::PositionMismatch);
    }
    let lp = DealerLpPositionV3::decode(observation.data)?;
    if lp.release_set != context.release_set
        || lp.market != context.market
        || lp.child_root != context.child_root
        || lp.lp_owner != lp_owner
        || lp.obligation_account != context.obligation_account
        || lp.generation != context.generation
    {
        return Err(MultiLpErrorV3::PositionMismatch);
    }
    Ok(lp)
}

fn prepare_custody_transfer(
    context: MultiLpContextV3,
    frame: MultiLpCollateralFrameV3,
    action: MultiLpActionV3,
    amount: u64,
    external_after: u64,
    principal_after: u64,
) -> MultiLpResultV3<MultiLpCustodyEffectV3> {
    let (
        source,
        destination,
        source_compartment,
        destination_compartment,
        source_owner,
        destination_owner,
        source_vault,
        destination_vault,
    ) = match action {
        MultiLpActionV3::Add => (
            frame.lp_external_account,
            frame.principal_vault,
            CompartmentV1::External,
            CompartmentV1::TradingPrincipal,
            frame.lp_owner,
            [0; 32],
            [0; 32],
            context.child_root,
        ),
        MultiLpActionV3::Remove => (
            frame.principal_vault,
            frame.lp_external_account,
            CompartmentV1::TradingPrincipal,
            CompartmentV1::External,
            [0; 32],
            frame.lp_owner,
            context.child_root,
            [0; 32],
        ),
    };
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
            order: frame.lp_owner,
            parent_request_digest: context.parent_request_digest,
            order_nonce: context.custody_replay_revision,
            generation: context.generation,
            page_index: 0,
            execution_index: 0,
            transfer_index: 0,
        },
        source,
        destination,
        source_vault_context: source_vault,
        destination_vault_context: destination_vault,
        mint: context.mint,
        token_program: context.token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision: context.custody_replay_revision,
        resulting_revision: context
            .custody_replay_revision
            .checked_add(1)
            .ok_or(MultiLpErrorV3::Arithmetic)?,
        amount,
        rent_lamports: 0,
    };
    request.validate().map_err(|_| MultiLpErrorV3::Custody)?;
    Ok(MultiLpCustodyEffectV3 {
        request,
        external_after,
        principal_after,
    })
}

fn read_identity(bytes: &[u8], offset: usize) -> MultiLpResultV3<[u8; 32]> {
    let value = bytes
        .get(offset..offset + 32)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(MultiLpErrorV3::InvalidState)?;
    if value == [0; 32] {
        Err(MultiLpErrorV3::InvalidState)
    } else {
        Ok(value)
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> MultiLpResultV3<u16> {
    bytes
        .get(offset..offset + 2)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(MultiLpErrorV3::InvalidState)
}

fn read_u64(bytes: &[u8], offset: usize) -> MultiLpResultV3<u64> {
    bytes
        .get(offset..offset + 8)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(MultiLpErrorV3::InvalidState)
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) -> MultiLpResultV3<()> {
    bytes
        .get_mut(offset..offset + 8)
        .ok_or(MultiLpErrorV3::InvalidState)?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::v3_obligation::{
        DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
        DEALER_OBLIGATION_VERSION_V3, DealerObligationProjectionV3,
    };
    use super::*;

    fn obligation_bytes(values: &[u64], lp: u64) -> std::vec::Vec<u8> {
        let mut bytes = std::vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + values.len() * 8];
        bytes[..8].copy_from_slice(&DEALER_OBLIGATION_MAGIC_V3);
        bytes[8..10].copy_from_slice(&DEALER_OBLIGATION_VERSION_V3.to_le_bytes());
        bytes[12..16].copy_from_slice(&(values.len() as u32).to_le_bytes());
        bytes[16..24].copy_from_slice(&9_u64.to_le_bytes());
        for (offset, value) in [
            (24, [1; 32]),
            (56, [2; 32]),
            (88, [3; 32]),
            (120, [4; 32]),
            (152, [5; 32]),
        ] {
            bytes[offset..offset + 32].copy_from_slice(&value);
        }
        bytes[184..192].copy_from_slice(&lp.to_le_bytes());
        for (index, value) in values.iter().enumerate() {
            let offset = DEALER_OBLIGATION_HEADER_BYTES_V3 + index * 8;
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn lp_bytes(shares: u64, trading: [u8; 32]) -> ([u8; 32], [u8; DEALER_LP_POSITION_BYTES_V3]) {
        let owner = [8; 32];
        let address = Pubkey::find_program_address(
            &[DEALER_LP_POSITION_PDA_DOMAIN_V3, &[5; 32], &owner],
            &Pubkey::new_from_array(trading),
        )
        .0
        .to_bytes();
        let obligation = Pubkey::find_program_address(
            &[
                super::super::v3_obligation::DEALER_OBLIGATION_PDA_DOMAIN_V3,
                &[5; 32],
            ],
            &Pubkey::new_from_array(trading),
        )
        .0
        .to_bytes();
        let mut bytes = [0; DEALER_LP_POSITION_BYTES_V3];
        DealerLpPositionV3 {
            revision: 4,
            release_set: [6; 32],
            market: [1; 32],
            child_root: [5; 32],
            lp_owner: owner,
            rent_refund: owner,
            obligation_account: obligation,
            principal_shares: shares,
            generation: 2,
        }
        .encode_into(&mut bytes)
        .expect("lp");
        (address, bytes)
    }

    fn context(trading: [u8; 32], custody: [u8; 32]) -> MultiLpContextV3 {
        let obligation_account = Pubkey::find_program_address(
            &[
                super::super::v3_obligation::DEALER_OBLIGATION_PDA_DOMAIN_V3,
                &[5; 32],
            ],
            &Pubkey::new_from_array(trading),
        )
        .0
        .to_bytes();
        MultiLpContextV3 {
            trading_program: trading,
            custody_program: custody,
            release_set: [6; 32],
            market: [1; 32],
            realm: [7; 32],
            child_root: [5; 32],
            obligation_account,
            mint: [10; 32],
            token_program: [11; 32],
            parent_request_digest: [12; 32],
            generation: 2,
            custody_replay_revision: 8,
            locked_capital_floor: 5,
        }
    }

    fn run(
        action: MultiLpActionV3,
        amount: u64,
    ) -> MultiLpResultV3<(
        MultiLpPlanV3,
        std::vec::Vec<u8>,
        [u8; DEALER_LP_POSITION_BYTES_V3],
    )> {
        let trading = [9; 32];
        let custody = [13; 32];
        let context = context(trading, custody);
        let principal_vault = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new([1; 32], [6; 32], [5; 32], CompartmentV1::TradingPrincipal)
                .as_slices(),
            &Pubkey::new_from_array(custody),
        )
        .0
        .to_bytes();
        let obligations = obligation_bytes(&[20, 20, 20], 20);
        let projection = DealerObligationProjectionV3::decode(&obligations).expect("obligations");
        let (lp_address, lp) = lp_bytes(20, trading);
        let frame = MultiLpCollateralFrameV3 {
            lp_external_account: [14; 32],
            lp_owner: [8; 32],
            lp_external_balance: 100,
            principal_vault,
            principal_balance: 30,
        };
        let claims = ClaimsInventoryObservation {
            market_id: [1; 32],
            product_id: [2; 32],
            liability_basis_id: [3; 32],
            position_owner: [4; 32],
            revision: 5,
            inventory: &[0, 0, 0],
        };
        let mut before = [0; 3];
        let mut after = [0; 3];
        let mut eq_before = [0; 3];
        let mut eq_after = [0; 3];
        let mut post_obligation = std::vec![0; obligations.len()];
        let mut post_lp = [0; DEALER_LP_POSITION_BYTES_V3];
        let plan = prepare_multi_lp_v3(
            context,
            frame,
            DealerLpAccountObservationV3 {
                address: lp_address,
                owner: trading,
                data: &lp,
            },
            projection,
            claims,
            MultiLpIntentV3 {
                action,
                amount,
                expected_lp_revision: 4,
                expected_lp_digest: hash(&lp).to_bytes(),
            },
            &mut before,
            &mut after,
            &mut eq_before,
            &mut eq_after,
            &mut post_obligation,
            &mut post_lp,
        )?;
        Ok((plan, post_obligation, post_lp))
    }

    #[test]
    fn add_and_remove_move_capital_and_uniform_obligations_together() {
        let (add, add_obligation, add_lp) = run(MultiLpActionV3::Add, 7).expect("add");
        assert_eq!(add.custody.external_after, 93);
        assert_eq!(add.custody.principal_after, 37);
        assert_eq!(
            DealerLpPositionV3::decode(&add_lp)
                .expect("lp")
                .principal_shares,
            27
        );
        assert_eq!(
            DealerObligationProjectionV3::decode(&add_obligation)
                .expect("obligation")
                .obligations()
                .collect::<std::vec::Vec<_>>(),
            [27, 27, 27]
        );
        assert_eq!(
            add.solvency_before.minimum_equity,
            add.solvency_after.minimum_equity
        );

        let (remove, remove_obligation, remove_lp) =
            run(MultiLpActionV3::Remove, 7).expect("remove");
        assert_eq!(remove.custody.external_after, 107);
        assert_eq!(remove.custody.principal_after, 23);
        assert_eq!(
            DealerLpPositionV3::decode(&remove_lp)
                .expect("lp")
                .principal_shares,
            13
        );
        assert_eq!(
            DealerObligationProjectionV3::decode(&remove_obligation)
                .expect("obligation")
                .obligations()
                .collect::<std::vec::Vec<_>>(),
            [13, 13, 13]
        );
        assert_eq!(
            remove.solvency_before.minimum_equity,
            remove.solvency_after.minimum_equity
        );
    }

    #[test]
    fn underflow_refusal_keeps_all_candidate_outputs_untouched() {
        let trading = [9; 32];
        let custody = [13; 32];
        let context = context(trading, custody);
        let principal_vault = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new([1; 32], [6; 32], [5; 32], CompartmentV1::TradingPrincipal)
                .as_slices(),
            &Pubkey::new_from_array(custody),
        )
        .0
        .to_bytes();
        let obligations = obligation_bytes(&[20, 20, 20], 20);
        let projection = DealerObligationProjectionV3::decode(&obligations).expect("obligations");
        let (lp_address, lp) = lp_bytes(20, trading);
        let claims = ClaimsInventoryObservation {
            market_id: [1; 32],
            product_id: [2; 32],
            liability_basis_id: [3; 32],
            position_owner: [4; 32],
            revision: 5,
            inventory: &[0, 0, 0],
        };
        let mut before = [77; 3];
        let mut after = [77; 3];
        let mut eq_before = [77; 3];
        let mut eq_after = [77; 3];
        let mut post_obligation = std::vec![0xa5; obligations.len()];
        let mut post_lp = [0xa5; DEALER_LP_POSITION_BYTES_V3];
        let result = prepare_multi_lp_v3(
            context,
            MultiLpCollateralFrameV3 {
                lp_external_account: [14; 32],
                lp_owner: [8; 32],
                lp_external_balance: 100,
                principal_vault,
                principal_balance: 30,
            },
            DealerLpAccountObservationV3 {
                address: lp_address,
                owner: trading,
                data: &lp,
            },
            projection,
            claims,
            MultiLpIntentV3 {
                action: MultiLpActionV3::Remove,
                amount: 21,
                expected_lp_revision: 4,
                expected_lp_digest: hash(&lp).to_bytes(),
            },
            &mut before,
            &mut after,
            &mut eq_before,
            &mut eq_after,
            &mut post_obligation,
            &mut post_lp,
        );
        assert_eq!(result, Err(MultiLpErrorV3::Arithmetic));
        assert!(post_obligation.iter().all(|byte| *byte == 0xa5));
        assert_eq!(post_lp, [0xa5; DEALER_LP_POSITION_BYTES_V3]);
    }
}
