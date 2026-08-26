//! Scenario-solvent, custody-backed multi-LP capital for Dealer V3.
//!
//! Each liquidity provider owns one Trading PDA whose `equity_shares` are a
//! junior pro-rata claim on the exact scenario-residual vector
//! `capital + Claims - obligations`. Shares are never par obligations. Later
//! issuance requires an exactly proportional scenario contribution; burns
//! return the floor-rounded pro-rata residual and leave rounding dust in the
//! pool. Fees, future order flow, and Market Hoard principal are absent.

use dclutch_custody_contract::{
    CUSTODY_REQUEST_BYTES_V1, CallerRoleV1, CompartmentV1, ContextV1, CustodyAuthoritySeedsV1,
    CustodyReceiptV1, CustodyRequestV1, CustodyVaultSeedsV1, DELEGATED_CUSTODY_RECEIPT_BYTES_V2,
    DELEGATED_CUSTODY_REQUEST_BYTES_V2, DelegatedCustodyReceiptV2, DelegatedCustodyRequestV2,
    OperationV1,
};
use dclutch_dealer_codec::scenario::{ClaimsInventoryObservation, ScenarioSolvencyReport};
use solana_program::{hash::hash, pubkey::Pubkey};

use super::v3_equity::{
    PoolEquityActionV3, PoolEquityContributionV3, PoolEquityInputV3, PoolEquityPlanV3,
    PoolEquityRedemptionV3, plan_pool_equity_v3,
};
use super::v3_obligation::{
    DealerObligationProjectionV3, EquityShareDeltaV3, ObligationErrorV3,
    stage_equity_share_supply_v3,
};

/// PDA domain for one LP position beneath a canonical Dealer child root.
pub const DEALER_LP_POSITION_PDA_DOMAIN_V3: &[u8] = b"dclutch:dealer-lp-position:v3";
/// Exact fixed LP position bytes.
pub const DEALER_LP_POSITION_BYTES_V3: usize = 256;
/// Exact LP position wire magic.
pub const DEALER_LP_POSITION_MAGIC_V3: [u8; 8] = *b"DCLDLP03";
/// Current LP position wire version.
pub const DEALER_LP_POSITION_VERSION_V3: u16 = 2;

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

/// Maximum physical Custody transfers in one equity contribution/redemption.
pub const MAX_MULTI_LP_CUSTODY_EFFECTS_V3: usize = 3;

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
    /// Exact outstanding junior equity shares owned by this LP.
    pub equity_shares: u64,
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
            equity_shares: read_u64(bytes, 216)?,
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
        output
            .get_mut(..8)
            .ok_or(MultiLpErrorV3::InvalidState)?
            .copy_from_slice(&DEALER_LP_POSITION_MAGIC_V3);
        output
            .get_mut(8..10)
            .ok_or(MultiLpErrorV3::InvalidState)?
            .copy_from_slice(&DEALER_LP_POSITION_VERSION_V3.to_le_bytes());
        write_u64(output, 16, self.revision)?;
        for (offset, value) in [
            (24, self.release_set),
            (56, self.market),
            (88, self.child_root),
            (120, self.lp_owner),
            (152, self.rent_refund),
            (184, self.obligation_account),
        ] {
            output
                .get_mut(offset..offset + 32)
                .ok_or(MultiLpErrorV3::InvalidState)?
                .copy_from_slice(&value);
        }
        write_u64(output, 216, self.equity_shares)?;
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
#[allow(clippy::too_many_arguments)]
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
        equity_shares: 0,
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

/// Prepare closure only after all junior equity shares have exited.
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
        || lp.equity_shares != 0
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
    /// Exact Custody delegate currently installed on the external account.
    pub lp_external_delegate: [u8; 32],
    /// Exact remaining delegated allowance; contributions exhaust it atomically.
    pub lp_external_delegated_amount: u64,
    /// Canonical TradingPrincipal vault.
    pub principal_vault: [u8; 32],
    /// TradingPrincipal balance before the operation.
    pub principal_balance: u64,
    /// Canonical Market HoardPrincipal vault used only for complete-set backing.
    pub hoard_vault: [u8; 32],
    /// HoardPrincipal balance before the operation.
    pub hoard_balance: u64,
}

/// User economic intent after chain-derived optimistic coordinates are fixed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiLpIntentV3<'a> {
    /// Contribute an exact scenario basket and mint junior equity shares.
    Contribute {
        /// Present collateral supplied by the LP.
        collateral: u64,
        /// Native Claims supplied by the LP in every scenario.
        claims: &'a [u64],
        /// Exact shares requested; later issuance must be exactly proportional.
        minted_shares: u64,
        /// Expected LP Position revision.
        expected_lp_revision: u64,
        /// Digest of exact LP Position prestate bytes.
        expected_lp_digest: [u8; 32],
    },
    /// Burn junior equity shares for the floor-rounded pro-rata residual.
    Redeem {
        /// Exact LP shares burned.
        burned_shares: u64,
        /// Expected LP Position revision.
        expected_lp_revision: u64,
        /// Digest of exact LP Position prestate bytes.
        expected_lp_digest: [u8; 32],
    },
}

impl MultiLpIntentV3<'_> {
    const fn action(self) -> MultiLpActionV3 {
        match self {
            Self::Contribute { .. } => MultiLpActionV3::Add,
            Self::Redeem { .. } => MultiLpActionV3::Remove,
        }
    }

    const fn expected_lp_revision(self) -> u64 {
        match self {
            Self::Contribute {
                expected_lp_revision,
                ..
            }
            | Self::Redeem {
                expected_lp_revision,
                ..
            } => expected_lp_revision,
        }
    }

    const fn expected_lp_digest(self) -> [u8; 32] {
        match self {
            Self::Contribute {
                expected_lp_digest, ..
            }
            | Self::Redeem {
                expected_lp_digest, ..
            } => expected_lp_digest,
        }
    }
}

/// Exact canonical Custody request selected for one pool effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiLpCustodyRequestV3 {
    /// Ordinary Custody V1 route whose source is already Custody-owned.
    Canonical(CustodyRequestV1),
    /// External LP debit that exhausts one exact delegated allowance.
    Delegated(DelegatedCustodyRequestV2),
}

impl MultiLpCustodyRequestV3 {
    /// Borrow the nested canonical Custody coordinates.
    pub const fn custody(self) -> CustodyRequestV1 {
        match self {
            Self::Canonical(request) => request,
            Self::Delegated(request) => request.custody,
        }
    }

    /// Exact child request width selected by its distinct magic.
    pub const fn encoded_len(self) -> usize {
        match self {
            Self::Canonical(_) => CUSTODY_REQUEST_BYTES_V1,
            Self::Delegated(_) => DELEGATED_CUSTODY_REQUEST_BYTES_V2,
        }
    }

    /// Encode into one exact caller-owned request buffer.
    pub fn encode_into(self, output: &mut [u8]) -> MultiLpResultV3<()> {
        if output.len() != self.encoded_len() {
            return Err(MultiLpErrorV3::Custody);
        }
        match self {
            Self::Canonical(request) => {
                output.copy_from_slice(&request.to_bytes().map_err(|_| MultiLpErrorV3::Custody)?)
            }
            Self::Delegated(request) => {
                output.copy_from_slice(&request.encode().map_err(|_| MultiLpErrorV3::Custody)?)
            }
        }
        Ok(())
    }
}

/// Exact Custody transfer and post-balance evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiLpCustodyEffectV3 {
    /// Canonical Custody request.
    pub request: MultiLpCustodyRequestV3,
    /// Required source balance after execution.
    pub source_after: u64,
    /// Required destination balance after execution.
    pub destination_after: u64,
}

/// Complete preflighted multi-LP candidate; Trading-owned writes commit last.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiLpPlanV3 {
    /// Selected action.
    pub action: MultiLpActionV3,
    /// Exact LP owner.
    pub lp_owner: [u8; 32],
    /// Exact shares minted or burned.
    pub share_delta: u64,
    /// Present collateral contributed by the LP.
    pub collateral_in: u64,
    /// Present collateral returned to the LP.
    pub collateral_out: u64,
    /// Minimum complete sets split for physical redemption.
    pub minimum_complete_sets_to_split: u64,
    /// Maximum complete sets merged after the Claims move.
    pub maximum_complete_sets_to_merge: u64,
    /// Incoming scenario-solvency report.
    pub solvency_before: ScenarioSolvencyReport,
    /// Candidate scenario-solvency report.
    pub solvency_after: ScenarioSolvencyReport,
    /// Exact ordered physical Custody effects.
    pub custody: [Option<MultiLpCustodyEffectV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3],
    /// Active prefix of the Custody effect bank.
    pub custody_count: u8,
    /// Required external collateral balance after execution.
    pub external_after: u64,
    /// Required TradingPrincipal balance after execution.
    pub principal_after: u64,
    /// Required HoardPrincipal balance after execution.
    pub hoard_after: u64,
    /// Required post-obligation-state digest.
    pub obligation_digest_after: [u8; 32],
    /// Required post-LP-state digest.
    pub lp_digest_after: [u8; 32],
    /// Required next obligation revision.
    pub obligation_revision_after: u64,
    /// Required next LP Position revision.
    pub lp_revision_after: u64,
    /// Exact total junior-equity share supply after this action.
    pub total_equity_shares_after: u64,
    /// Exact executing LP's junior-equity share balance after this action.
    pub lp_equity_shares_after: u64,
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
    dealer_claims_position: ClaimsInventoryObservation<'_>,
    lp_claims_position: ClaimsInventoryObservation<'_>,
    intent: MultiLpIntentV3<'_>,
    obligation_scratch: &mut [u64],
    residual_before: &mut [u64],
    residual_after: &mut [u64],
    claims_transferred: &mut [u64],
    post_dealer_claims: &mut [u64],
    post_lp_claims: &mut [u64],
    post_obligation: &mut [u8],
    post_lp: &mut [u8],
) -> MultiLpResultV3<MultiLpPlanV3> {
    validate_context(
        context,
        collateral,
        obligation,
        dealer_claims_position,
        lp_claims_position,
    )?;
    if intent.expected_lp_digest() == [0; 32]
        || hash(lp_account.data).to_bytes() != intent.expected_lp_digest()
        || collateral.lp_owner == [0; 32]
    {
        return Err(MultiLpErrorV3::InvalidState);
    }
    let lp = authenticate_lp_position(context, collateral.lp_owner, lp_account)?;
    if lp.revision != intent.expected_lp_revision()
        || lp.equity_shares > obligation.total_equity_shares()
        || lp.revision == u64::MAX
        || obligation.revision() == u64::MAX
    {
        return Err(MultiLpErrorV3::PositionMismatch);
    }
    let width = usize::try_from(obligation.width()).map_err(|_| MultiLpErrorV3::WidthMismatch)?;
    for observed in [
        dealer_claims_position.inventory.len(),
        lp_claims_position.inventory.len(),
        obligation_scratch.len(),
        residual_before.len(),
        residual_after.len(),
        claims_transferred.len(),
        post_dealer_claims.len(),
        post_lp_claims.len(),
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

    let equity_action = match intent {
        MultiLpIntentV3::Contribute {
            collateral: contribution,
            claims,
            minted_shares,
            ..
        } => {
            if contribution > collateral.lp_external_balance || claims.len() != width {
                return Err(MultiLpErrorV3::Arithmetic);
            }
            for (available, supplied) in lp_claims_position.inventory.iter().zip(claims.iter()) {
                if supplied > available {
                    return Err(MultiLpErrorV3::Arithmetic);
                }
            }
            PoolEquityActionV3::Contribute(PoolEquityContributionV3 {
                collateral: contribution,
                claims,
                minted_shares,
            })
        }
        MultiLpIntentV3::Redeem { burned_shares, .. } => {
            if burned_shares > lp.equity_shares {
                return Err(MultiLpErrorV3::Arithmetic);
            }
            for (lp_inventory, dealer_inventory) in lp_claims_position
                .inventory
                .iter()
                .zip(dealer_claims_position.inventory.iter())
            {
                let largest_residual = collateral
                    .principal_balance
                    .checked_add(*dealer_inventory)
                    .ok_or(MultiLpErrorV3::Arithmetic)?;
                lp_inventory
                    .checked_add(largest_residual)
                    .ok_or(MultiLpErrorV3::Arithmetic)?;
            }
            PoolEquityActionV3::Redeem(PoolEquityRedemptionV3 { burned_shares })
        }
    };
    for (destination, value) in obligation_scratch.iter_mut().zip(obligation.obligations()) {
        *destination = value;
    }
    let equity = plan_pool_equity_v3(
        PoolEquityInputV3 {
            collateral: collateral.principal_balance,
            claims: dealer_claims_position.inventory,
            obligations: obligation_scratch,
            total_shares: obligation.total_equity_shares(),
            locked_capital_floor: context.locked_capital_floor,
            action: equity_action,
        },
        residual_before,
        residual_after,
        claims_transferred,
        post_dealer_claims,
    )
    .map_err(|error| match error {
        super::v3_equity::PoolEquityErrorV3::Insolvent => MultiLpErrorV3::Insolvent,
        super::v3_equity::PoolEquityErrorV3::WidthMismatch => MultiLpErrorV3::WidthMismatch,
        _ => MultiLpErrorV3::Arithmetic,
    })?;
    let (next_lp_shares, share_delta) = match intent {
        MultiLpIntentV3::Contribute { claims, .. } => {
            for ((output, current), supplied) in post_lp_claims
                .iter_mut()
                .zip(lp_claims_position.inventory.iter())
                .zip(claims.iter())
            {
                *output = current.saturating_sub(*supplied);
            }
            (
                lp.equity_shares
                    .checked_add(equity.share_delta)
                    .ok_or(MultiLpErrorV3::Arithmetic)?,
                EquityShareDeltaV3::Mint(equity.share_delta),
            )
        }
        MultiLpIntentV3::Redeem { .. } => {
            for ((output, current), received) in post_lp_claims
                .iter_mut()
                .zip(lp_claims_position.inventory.iter())
                .zip(claims_transferred.iter())
            {
                *output = current.saturating_add(*received);
            }
            (
                lp.equity_shares
                    .checked_sub(equity.share_delta)
                    .ok_or(MultiLpErrorV3::Arithmetic)?,
                EquityShareDeltaV3::Burn(equity.share_delta),
            )
        }
    };
    let external_after = collateral
        .lp_external_balance
        .checked_sub(equity.collateral_in)
        .and_then(|value| value.checked_add(equity.collateral_out))
        .ok_or(MultiLpErrorV3::Arithmetic)?;
    let (custody, custody_count, hoard_after) =
        prepare_equity_custody_sequence(context, collateral, equity, external_after)?;

    let next_lp = DealerLpPositionV3 {
        revision: lp
            .revision
            .checked_add(1)
            .ok_or(MultiLpErrorV3::Arithmetic)?,
        equity_shares: next_lp_shares,
        ..lp
    };
    let mut staged_lp = [0; DEALER_LP_POSITION_BYTES_V3];
    next_lp.encode_into(&mut staged_lp)?;
    stage_equity_share_supply_v3(obligation, share_delta, post_obligation)?;
    let staged_projection = DealerObligationProjectionV3::decode(post_obligation)?;
    post_lp.copy_from_slice(&staged_lp);
    let solvency_before = ScenarioSolvencyReport {
        minimum_equity: i128::from(equity.minimum_residual_before),
        minimum_scenario: equity.minimum_scenario_before,
        present_capital: collateral.principal_balance,
        locked_capital_floor: context.locked_capital_floor,
    };
    let solvency_after = ScenarioSolvencyReport {
        minimum_equity: i128::from(equity.minimum_residual_after),
        minimum_scenario: equity.minimum_scenario_after,
        present_capital: equity.collateral_after,
        locked_capital_floor: context.locked_capital_floor,
    };
    Ok(MultiLpPlanV3 {
        action: intent.action(),
        lp_owner: collateral.lp_owner,
        share_delta: equity.share_delta,
        collateral_in: equity.collateral_in,
        collateral_out: equity.collateral_out,
        minimum_complete_sets_to_split: equity.minimum_complete_sets_to_split,
        maximum_complete_sets_to_merge: equity.maximum_complete_sets_to_merge,
        solvency_before,
        solvency_after,
        custody,
        custody_count,
        external_after,
        principal_after: equity.collateral_after,
        hoard_after,
        obligation_digest_after: hash(post_obligation).to_bytes(),
        lp_digest_after: hash(&staged_lp).to_bytes(),
        obligation_revision_after: staged_projection.revision(),
        lp_revision_after: next_lp.revision,
        total_equity_shares_after: staged_projection.total_equity_shares(),
        lp_equity_shares_after: next_lp.equity_shares,
    })
}

/// Verify one immediate Custody receipt in the admitted global route order.
pub fn verify_multi_lp_custody_receipt_v3(
    plan: MultiLpPlanV3,
    index: u8,
    custody_receipt: &[u8],
    custody_poststate_commitment: [u8; 32],
) -> MultiLpResultV3<()> {
    if index >= plan.custody_count {
        return Err(MultiLpErrorV3::Custody);
    }
    let effect = plan
        .custody
        .get(usize::from(index))
        .copied()
        .flatten()
        .ok_or(MultiLpErrorV3::Custody)?;
    let mut request_bytes = [0_u8; DELEGATED_CUSTODY_REQUEST_BYTES_V2];
    let request_slice = request_bytes
        .get_mut(..effect.request.encoded_len())
        .ok_or(MultiLpErrorV3::Custody)?;
    effect.request.encode_into(request_slice)?;
    let request_digest = hash(request_slice).to_bytes();
    let evidence = match effect.request {
        MultiLpCustodyRequestV3::Canonical(request) => {
            let receipt =
                CustodyReceiptV1::decode(custody_receipt).map_err(|_| MultiLpErrorV3::Custody)?;
            receipt
                .verify_for(request, request_digest, custody_poststate_commitment)
                .map_err(|_| MultiLpErrorV3::Custody)?;
            receipt.evidence
        }
        MultiLpCustodyRequestV3::Delegated(request) => {
            if custody_receipt.len() != DELEGATED_CUSTODY_RECEIPT_BYTES_V2 {
                return Err(MultiLpErrorV3::Custody);
            }
            let receipt = DelegatedCustodyReceiptV2::decode(custody_receipt)
                .map_err(|_| MultiLpErrorV3::Custody)?;
            if receipt.starts_atomic_debit != request.starts_atomic_debit
                || receipt.terminal != request.terminal
                || receipt.delegate_before != request.delegate_before
                || receipt.delegate_after != request.delegate_after
                || receipt.total_debit != request.total_debit
                || receipt.allowance_before != request.allowance_before
                || receipt.allowance_after != request.allowance_after
            {
                return Err(MultiLpErrorV3::Custody);
            }
            receipt
                .custody
                .verify_for(
                    request.custody,
                    request_digest,
                    custody_poststate_commitment,
                )
                .map_err(|_| MultiLpErrorV3::Custody)?;
            receipt.custody.evidence
        }
    };
    if evidence.source_after != effect.source_after
        || evidence.destination_after != effect.destination_after
    {
        return Err(MultiLpErrorV3::Postcondition);
    }
    Ok(())
}

/// Verify all write-last pool, share, and Claims postconditions.
#[allow(clippy::too_many_arguments)]
pub fn verify_multi_lp_postconditions_v3(
    plan: MultiLpPlanV3,
    observed_external_balance: u64,
    observed_principal_balance: u64,
    observed_hoard_balance: u64,
    observed_obligation: &[u8],
    observed_lp: &[u8],
    expected_dealer_claims: &[u64],
    observed_dealer_claims: &[u64],
    expected_lp_claims: &[u64],
    observed_lp_claims: &[u64],
) -> MultiLpResultV3<()> {
    if observed_external_balance != plan.external_after
        || observed_principal_balance != plan.principal_after
        || observed_hoard_balance != plan.hoard_after
        || hash(observed_obligation).to_bytes() != plan.obligation_digest_after
        || hash(observed_lp).to_bytes() != plan.lp_digest_after
        || DealerObligationProjectionV3::decode(observed_obligation)?.revision()
            != plan.obligation_revision_after
        || DealerLpPositionV3::decode(observed_lp)?.revision != plan.lp_revision_after
        || expected_dealer_claims != observed_dealer_claims
        || expected_lp_claims != observed_lp_claims
        || expected_dealer_claims.len() != expected_lp_claims.len()
    {
        return Err(MultiLpErrorV3::Postcondition);
    }
    Ok(())
}

fn validate_context(
    context: MultiLpContextV3,
    collateral: MultiLpCollateralFrameV3,
    obligation: DealerObligationProjectionV3<'_>,
    dealer_claims: ClaimsInventoryObservation<'_>,
    lp_claims: ClaimsInventoryObservation<'_>,
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
        collateral.hoard_vault,
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
    let descriptor = obligation.descriptor(context.locked_capital_floor);
    let expected_hoard = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            context.market,
            context.release_set,
            context.market,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &Pubkey::new_from_array(context.custody_program),
    )
    .0
    .to_bytes();
    if context.obligation_account != expected_obligation
        || collateral.principal_vault != expected_vault
        || collateral.hoard_vault != expected_hoard
        || collateral.principal_vault == collateral.lp_external_account
        || collateral.hoard_vault == collateral.lp_external_account
        || collateral.hoard_vault == collateral.principal_vault
        || obligation.child_root() != context.child_root
        || obligation.position_owner() != dealer_claims.position_owner
        || descriptor.market_id != context.market
        || descriptor.product_id != dealer_claims.product_id
        || descriptor.liability_basis_id != dealer_claims.liability_basis_id
        || usize::try_from(obligation.width()).ok() != Some(dealer_claims.inventory.len())
        || dealer_claims.inventory.len() != lp_claims.inventory.len()
        || dealer_claims.market_id != context.market
        || lp_claims.market_id != context.market
        || dealer_claims.product_id != lp_claims.product_id
        || dealer_claims.liability_basis_id != lp_claims.liability_basis_id
        || lp_claims.position_owner != collateral.lp_owner
        || dealer_claims.position_owner == lp_claims.position_owner
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EquityCustodyTransferV3 {
    ExternalToPrincipal,
    PrincipalToExternal,
    PrincipalToHoard,
    HoardToPrincipal,
}

fn prepare_equity_custody_sequence(
    context: MultiLpContextV3,
    frame: MultiLpCollateralFrameV3,
    plan: PoolEquityPlanV3,
    external_after: u64,
) -> MultiLpResultV3<(
    [Option<MultiLpCustodyEffectV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3],
    u8,
    u64,
)> {
    let mut effects = [None; MAX_MULTI_LP_CUSTODY_EFFECTS_V3];
    let mut count = 0_usize;
    let mut external = frame.lp_external_balance;
    let mut principal = frame.principal_balance;
    let mut hoard = frame.hoard_balance;
    if plan.collateral_in != 0 {
        stage_equity_custody_transfer(
            context,
            frame,
            EquityCustodyTransferV3::ExternalToPrincipal,
            plan.collateral_in,
            &mut external,
            &mut principal,
            &mut hoard,
            &mut effects,
            &mut count,
        )?;
    }
    if plan.minimum_complete_sets_to_split != 0 {
        stage_equity_custody_transfer(
            context,
            frame,
            EquityCustodyTransferV3::PrincipalToHoard,
            plan.minimum_complete_sets_to_split,
            &mut external,
            &mut principal,
            &mut hoard,
            &mut effects,
            &mut count,
        )?;
    }
    if plan.collateral_out != 0 {
        stage_equity_custody_transfer(
            context,
            frame,
            EquityCustodyTransferV3::PrincipalToExternal,
            plan.collateral_out,
            &mut external,
            &mut principal,
            &mut hoard,
            &mut effects,
            &mut count,
        )?;
    }
    if plan.maximum_complete_sets_to_merge != 0 {
        stage_equity_custody_transfer(
            context,
            frame,
            EquityCustodyTransferV3::HoardToPrincipal,
            plan.maximum_complete_sets_to_merge,
            &mut external,
            &mut principal,
            &mut hoard,
            &mut effects,
            &mut count,
        )?;
    }
    if count > effects.len() || external != external_after || principal != plan.collateral_after {
        return Err(MultiLpErrorV3::Postcondition);
    }
    Ok((
        effects,
        u8::try_from(count).map_err(|_| MultiLpErrorV3::Arithmetic)?,
        hoard,
    ))
}

#[allow(clippy::too_many_arguments)]
fn stage_equity_custody_transfer(
    context: MultiLpContextV3,
    frame: MultiLpCollateralFrameV3,
    kind: EquityCustodyTransferV3,
    amount: u64,
    external: &mut u64,
    principal: &mut u64,
    hoard: &mut u64,
    output: &mut [Option<MultiLpCustodyEffectV3>; MAX_MULTI_LP_CUSTODY_EFFECTS_V3],
    count: &mut usize,
) -> MultiLpResultV3<()> {
    if amount == 0 || *count >= output.len() {
        return Err(MultiLpErrorV3::Arithmetic);
    }
    let (source_before, destination_before) = match kind {
        EquityCustodyTransferV3::ExternalToPrincipal => (*external, *principal),
        EquityCustodyTransferV3::PrincipalToExternal => (*principal, *external),
        EquityCustodyTransferV3::PrincipalToHoard => (*principal, *hoard),
        EquityCustodyTransferV3::HoardToPrincipal => (*hoard, *principal),
    };
    let source_after = source_before
        .checked_sub(amount)
        .ok_or(MultiLpErrorV3::Arithmetic)?;
    let destination_after = destination_before
        .checked_add(amount)
        .ok_or(MultiLpErrorV3::Arithmetic)?;
    let (
        source,
        destination,
        source_compartment,
        destination_compartment,
        source_owner,
        destination_owner,
        source_vault,
        destination_vault,
    ) = match kind {
        EquityCustodyTransferV3::ExternalToPrincipal => (
            frame.lp_external_account,
            frame.principal_vault,
            CompartmentV1::External,
            CompartmentV1::TradingPrincipal,
            frame.lp_owner,
            [0; 32],
            [0; 32],
            context.child_root,
        ),
        EquityCustodyTransferV3::PrincipalToExternal => (
            frame.principal_vault,
            frame.lp_external_account,
            CompartmentV1::TradingPrincipal,
            CompartmentV1::External,
            [0; 32],
            frame.lp_owner,
            context.child_root,
            [0; 32],
        ),
        EquityCustodyTransferV3::PrincipalToHoard => (
            frame.principal_vault,
            frame.hoard_vault,
            CompartmentV1::TradingPrincipal,
            CompartmentV1::HoardPrincipal,
            [0; 32],
            [0; 32],
            context.child_root,
            context.market,
        ),
        EquityCustodyTransferV3::HoardToPrincipal => (
            frame.hoard_vault,
            frame.principal_vault,
            CompartmentV1::HoardPrincipal,
            CompartmentV1::TradingPrincipal,
            [0; 32],
            [0; 32],
            context.market,
            context.child_root,
        ),
    };
    let ordinal = u16::try_from(*count).map_err(|_| MultiLpErrorV3::Arithmetic)?;
    let expected_revision = context
        .custody_replay_revision
        .checked_add(u64::from(ordinal))
        .ok_or(MultiLpErrorV3::Arithmetic)?;
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
            transfer_index: ordinal,
        },
        source,
        destination,
        source_vault_context: source_vault,
        destination_vault_context: destination_vault,
        mint: context.mint,
        token_program: context.token_program,
        payer: [0; 32],
        rent_refund: [0; 32],
        expected_revision,
        resulting_revision: expected_revision
            .checked_add(1)
            .ok_or(MultiLpErrorV3::Arithmetic)?,
        amount,
        rent_lamports: 0,
    };
    request.validate().map_err(|_| MultiLpErrorV3::Custody)?;
    let request = if kind == EquityCustodyTransferV3::ExternalToPrincipal {
        let authority = Pubkey::find_program_address(
            &CustodyAuthoritySeedsV1::from_request(request).as_slices(),
            &Pubkey::new_from_array(context.custody_program),
        )
        .0
        .to_bytes();
        if frame.lp_external_delegate != authority || frame.lp_external_delegated_amount != amount {
            return Err(MultiLpErrorV3::Custody);
        }
        MultiLpCustodyRequestV3::Delegated(DelegatedCustodyRequestV2 {
            custody: request,
            starts_atomic_debit: true,
            terminal: true,
            delegate_before: authority,
            delegate_after: [0; 32],
            total_debit: amount,
            allowance_before: amount,
            allowance_after: 0,
        })
    } else {
        MultiLpCustodyRequestV3::Canonical(request)
    };
    *output.get_mut(*count).ok_or(MultiLpErrorV3::Custody)? = Some(MultiLpCustodyEffectV3 {
        request,
        source_after,
        destination_after,
    });
    match kind {
        EquityCustodyTransferV3::ExternalToPrincipal => {
            *external = source_after;
            *principal = destination_after;
        }
        EquityCustodyTransferV3::PrincipalToExternal => {
            *principal = source_after;
            *external = destination_after;
        }
        EquityCustodyTransferV3::PrincipalToHoard => {
            *principal = source_after;
            *hoard = destination_after;
        }
        EquityCustodyTransferV3::HoardToPrincipal => {
            *hoard = source_after;
            *principal = destination_after;
        }
    }
    *count = count.checked_add(1).ok_or(MultiLpErrorV3::Arithmetic)?;
    Ok(())
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
            equity_shares: shares,
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
    ) -> MultiLpResultV3<(
        MultiLpPlanV3,
        std::vec::Vec<u8>,
        [u8; DEALER_LP_POSITION_BYTES_V3],
        [u64; 3],
        [u64; 3],
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
        let hoard_vault = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new([1; 32], [6; 32], [1; 32], CompartmentV1::HoardPrincipal)
                .as_slices(),
            &Pubkey::new_from_array(custody),
        )
        .0
        .to_bytes();
        let obligations = obligation_bytes(&[0, 0, 0], 20);
        let projection = DealerObligationProjectionV3::decode(&obligations).expect("obligations");
        let (lp_address, lp) = lp_bytes(20, trading);
        let frame = MultiLpCollateralFrameV3 {
            lp_external_account: [14; 32],
            lp_owner: [8; 32],
            lp_external_balance: 100,
            lp_external_delegate: Pubkey::find_program_address(
                &[
                    dclutch_custody_contract::CUSTODY_AUTHORITY_PDA_DOMAIN_V1,
                    &context.market,
                    &context.release_set,
                ],
                &Pubkey::new_from_array(context.custody_program),
            )
            .0
            .to_bytes(),
            lp_external_delegated_amount: 10,
            principal_vault,
            principal_balance: 20,
            hoard_vault,
            hoard_balance: 100,
        };
        let dealer_claims = ClaimsInventoryObservation {
            market_id: [1; 32],
            product_id: [2; 32],
            liability_basis_id: [3; 32],
            position_owner: [4; 32],
            revision: 5,
            inventory: &[0, 10, 20],
        };
        let lp_claims = ClaimsInventoryObservation {
            market_id: [1; 32],
            product_id: [2; 32],
            liability_basis_id: [3; 32],
            position_owner: [8; 32],
            revision: 6,
            inventory: &[10, 10, 10],
        };
        let intent = match action {
            MultiLpActionV3::Add => MultiLpIntentV3::Contribute {
                collateral: 10,
                claims: &[0, 5, 10],
                minted_shares: 10,
                expected_lp_revision: 4,
                expected_lp_digest: hash(&lp).to_bytes(),
            },
            MultiLpActionV3::Remove => MultiLpIntentV3::Redeem {
                burned_shares: 10,
                expected_lp_revision: 4,
                expected_lp_digest: hash(&lp).to_bytes(),
            },
        };
        let mut obligations_scratch = [0; 3];
        let mut before = [0; 3];
        let mut after = [0; 3];
        let mut transferred = [0; 3];
        let mut post_dealer_claims = [0; 3];
        let mut post_lp_claims = [0; 3];
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
            dealer_claims,
            lp_claims,
            intent,
            &mut obligations_scratch,
            &mut before,
            &mut after,
            &mut transferred,
            &mut post_dealer_claims,
            &mut post_lp_claims,
            &mut post_obligation,
            &mut post_lp,
        )?;
        Ok((
            plan,
            post_obligation,
            post_lp,
            post_dealer_claims,
            post_lp_claims,
        ))
    }

    #[test]
    fn contributions_and_redemptions_move_junior_scenario_equity() {
        let (add, add_obligation, add_lp, add_dealer_claims, add_lp_claims) =
            run(MultiLpActionV3::Add).expect("proportional contribution");
        assert_eq!(add.external_after, 90);
        assert_eq!(add.principal_after, 30);
        assert_eq!(add_dealer_claims, [0, 15, 30]);
        assert_eq!(add_lp_claims, [10, 5, 0]);
        assert_eq!(
            DealerLpPositionV3::decode(&add_lp)
                .expect("lp")
                .equity_shares,
            30
        );
        assert_eq!(
            DealerObligationProjectionV3::decode(&add_obligation)
                .expect("obligation")
                .obligations()
                .collect::<std::vec::Vec<_>>(),
            [0, 0, 0]
        );
        assert_eq!(add.solvency_before.minimum_equity, 20);
        assert_eq!(add.solvency_after.minimum_equity, 30);

        let (remove, remove_obligation, remove_lp, remove_dealer_claims, remove_lp_claims) =
            run(MultiLpActionV3::Remove).expect("pro-rata redemption");
        assert_eq!(remove.external_after, 110);
        assert_eq!(remove.principal_after, 10);
        assert_eq!(remove_dealer_claims, [0, 5, 10]);
        assert_eq!(remove_lp_claims, [10, 15, 20]);
        assert_eq!(
            DealerLpPositionV3::decode(&remove_lp)
                .expect("lp")
                .equity_shares,
            10
        );
        assert_eq!(
            DealerObligationProjectionV3::decode(&remove_obligation)
                .expect("obligation")
                .obligations()
                .collect::<std::vec::Vec<_>>(),
            [0, 0, 0]
        );
        assert_eq!(remove.solvency_before.minimum_equity, 20);
        assert_eq!(remove.solvency_after.minimum_equity, 10);
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
        let hoard_vault = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new([1; 32], [6; 32], [1; 32], CompartmentV1::HoardPrincipal)
                .as_slices(),
            &Pubkey::new_from_array(custody),
        )
        .0
        .to_bytes();
        let obligations = obligation_bytes(&[0, 0, 0], 20);
        let projection = DealerObligationProjectionV3::decode(&obligations).expect("obligations");
        let (lp_address, lp) = lp_bytes(20, trading);
        let claims = ClaimsInventoryObservation {
            market_id: [1; 32],
            product_id: [2; 32],
            liability_basis_id: [3; 32],
            position_owner: [4; 32],
            revision: 5,
            inventory: &[0, 10, 20],
        };
        let lp_claims = ClaimsInventoryObservation {
            market_id: [1; 32],
            product_id: [2; 32],
            liability_basis_id: [3; 32],
            position_owner: [8; 32],
            revision: 6,
            inventory: &[0, 0, 0],
        };
        let mut obligations_scratch = [77; 3];
        let mut before = [77; 3];
        let mut after = [77; 3];
        let mut transferred = [77; 3];
        let mut post_dealer_claims = [77; 3];
        let mut post_lp_claims = [77; 3];
        let mut post_obligation = std::vec![0xa5; obligations.len()];
        let mut post_lp = [0xa5; DEALER_LP_POSITION_BYTES_V3];
        let result = prepare_multi_lp_v3(
            context,
            MultiLpCollateralFrameV3 {
                lp_external_account: [14; 32],
                lp_owner: [8; 32],
                lp_external_balance: 100,
                lp_external_delegate: [0; 32],
                lp_external_delegated_amount: 0,
                principal_vault,
                principal_balance: 20,
                hoard_vault,
                hoard_balance: 100,
            },
            DealerLpAccountObservationV3 {
                address: lp_address,
                owner: trading,
                data: &lp,
            },
            projection,
            claims,
            lp_claims,
            MultiLpIntentV3::Redeem {
                burned_shares: 21,
                expected_lp_revision: 4,
                expected_lp_digest: hash(&lp).to_bytes(),
            },
            &mut obligations_scratch,
            &mut before,
            &mut after,
            &mut transferred,
            &mut post_dealer_claims,
            &mut post_lp_claims,
            &mut post_obligation,
            &mut post_lp,
        );
        assert_eq!(result, Err(MultiLpErrorV3::Arithmetic));
        assert!(post_obligation.iter().all(|byte| *byte == 0xa5));
        assert_eq!(post_lp, [0xa5; DEALER_LP_POSITION_BYTES_V3]);
    }
}
