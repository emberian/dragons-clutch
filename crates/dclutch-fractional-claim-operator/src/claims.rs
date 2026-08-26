//! Chain-derived composition with the canonical Claims SignedDeltaV3 waist.

use dclutch_claims_svm::{
    frame_spec_v1::{SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3, SignedDeltaFrameSpecV3},
    liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2,
    protocol_position_v2::ProtocolPositionSeedsV2,
    signed_delta_v3::{
        DeltaDirectionV3, PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanV3, SignedDeltaV3,
    },
};
use dclutch_fractional_claim_kernel::OutcomeReserveV1;
use dclutch_fractional_claims_kernel::{
    FractionalSignedDeltaInputV1, FractionalSignedDeltaLoweringV1,
    fractional_signed_delta_shape_v1, lower_fractional_signed_delta_v1,
    validate_fractional_signed_delta_postcondition_v1,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::{
    Error, FractionalActionPlanV1, FractionalPreparedChainArtifactsV1, Result,
    authenticate_fractional_chain_artifacts_v1,
};

/// One exact canonical Claims Position observed from the same finalized snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalClaimsPositionSnapshotV1<'a> {
    /// Canonical Claims Position PDA.
    pub account: Pubkey,
    /// Exact canonical pre-state bytes.
    pub bytes: &'a [u8],
}

/// Chain-derived Claims state needed to lower one Fractional action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalSignedDeltaChainObservationV1<'a> {
    /// Canonical Claims aggregate PDA.
    pub market_account: Pubkey,
    /// Exact Claims aggregate pre-state.
    pub market_bytes: &'a [u8],
    /// Exact finalized linked-basis raw-record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Fractional-root-owned reserve Position.
    pub reserve: FractionalClaimsPositionSnapshotV1<'a>,
    /// Actor Position for wrap/whole unwrap; absent for terminal debit/retire.
    pub actor: Option<FractionalClaimsPositionSnapshotV1<'a>>,
}

/// Owned canonical Claims child plan and exact candidate post-resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalSignedDeltaChainPlanV1 {
    lowering: FractionalSignedDeltaLoweringV1,
    packet: Vec<u8>,
    market_account: Pubkey,
    ordered_position_accounts: Vec<Pubkey>,
    expected_post_market: Vec<u8>,
    expected_post_positions: Vec<Vec<u8>>,
    claims_program: Pubkey,
    trading_program: Pubkey,
}

impl FractionalSignedDeltaChainPlanV1 {
    /// Family economic postcondition and exact Claims commitments.
    pub const fn lowering(&self) -> FractionalSignedDeltaLoweringV1 {
        self.lowering
    }

    /// Exact canonical SignedDeltaV3 child packet.
    pub fn packet(&self) -> &[u8] {
        &self.packet
    }

    /// Canonical Claims aggregate account.
    pub const fn market_account(&self) -> Pubkey {
        self.market_account
    }

    /// Canonical Position PDAs in SignedDelta table order.
    pub fn ordered_position_accounts(&self) -> &[Pubkey] {
        &self.ordered_position_accounts
    }

    /// Exact expected post-commit Claims aggregate bytes.
    pub fn expected_post_market(&self) -> &[u8] {
        &self.expected_post_market
    }

    /// Exact expected post-commit Positions in SignedDelta table order.
    pub fn expected_post_positions(&self) -> &[Vec<u8>] {
        &self.expected_post_positions
    }

    /// Current Registry-selected Claims program.
    pub const fn claims_program(&self) -> Pubkey {
        self.claims_program
    }
}

/// Lower one already planned Fractional action through canonical SignedDeltaV3.
///
/// The reserve owner is always the authenticated Fractional root key. The
/// linked-basis digest and Claims state remain independently chain-derived and
/// are never copied from the caller's family request.
pub fn lower_fractional_action_to_signed_delta_v1(
    prepared: FractionalPreparedChainArtifactsV1<'_>,
    action: &FractionalActionPlanV1,
    observed: FractionalSignedDeltaChainObservationV1<'_>,
) -> Result<FractionalSignedDeltaChainPlanV1> {
    authenticate_fractional_chain_artifacts_v1(prepared, &action.request.to_bytes())?;
    let claims_program = prepared.checked_release().claims_program();
    let trading_program = prepared.checked_release().trading_program();
    let expected_market = Pubkey::find_program_address(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            action.request.input().market.as_slice(),
        ],
        &claims_program,
    )
    .0;
    if observed.market_account != expected_market {
        return Err(Error::Claims);
    }
    authenticate_position(
        claims_program,
        observed.market_account,
        prepared.root_key(),
        observed.reserve,
    )?;
    let actor = match observed.actor {
        Some(actor) => {
            authenticate_position(
                claims_program,
                observed.market_account,
                Pubkey::new_from_array(action.request.input().owner),
                actor,
            )?;
            Some(actor)
        }
        None => None,
    };
    let expected_post_reserve_native_claims = action
        .post_reserve
        .map(|reserve: OutcomeReserveV1| reserve.locked_native_claims);
    let input = FractionalSignedDeltaInputV1 {
        request: action.request,
        semantic_product_id: prepared.product_join().product_id.to_bytes(),
        market_account: observed.market_account.to_bytes(),
        market_bytes: observed.market_bytes,
        linked_basis_record_digest: observed.linked_basis_record_digest,
        claims_program: claims_program.to_bytes(),
        reserve_owner: prepared.root_key().to_bytes(),
        reserve_position_bytes: observed.reserve.bytes,
        actor_position_bytes: actor.map(|position| position.bytes),
        native_claims: action.native_claims,
        collateral_atoms: action.collateral_atoms,
        expected_post_reserve_native_claims,
        retirement_native_burns: &action.retirement_native_burns,
        post_fractional_revision: action.post_revision,
    };
    let shape = fractional_signed_delta_shape_v1(input).map_err(|_| Error::Claims)?;
    let neutral = SignedDeltaV3::new(DeltaDirectionV3::Neutral, 0).map_err(|_| Error::Claims)?;
    let mut aggregates =
        vec![neutral; usize::try_from(shape.claim_count()).map_err(|_| Error::Claims)?];
    let dummy = PositionDeltaV3::new(
        PositionDeltaInputV3 {
            position_index: 0,
            outcome: 0,
            delta: SignedDeltaV3::new(DeltaDirectionV3::Debit, 1).map_err(|_| Error::Claims)?,
        },
        shape.position_count(),
        shape.claim_count(),
    )
    .map_err(|_| Error::Claims)?;
    let mut rows =
        vec![dummy; usize::try_from(shape.position_delta_count()).map_err(|_| Error::Claims)?];
    let mut packet_scratch = vec![0; shape.packet_bytes()];
    let mut packet = vec![0; shape.packet_bytes()];
    let mut expected_post_market = vec![0; observed.market_bytes.len()];
    let mut expected_post_positions = vec![
        vec![0; observed.reserve.bytes.len()];
        usize::try_from(shape.position_count())
            .map_err(|_| Error::Claims)?
    ];
    let mut position_outputs: Vec<&mut [u8]> = expected_post_positions
        .iter_mut()
        .map(Vec::as_mut_slice)
        .collect();
    let lowering = lower_fractional_signed_delta_v1(
        input,
        &mut aggregates,
        &mut rows,
        &mut packet_scratch,
        &mut packet,
        &mut expected_post_market,
        &mut position_outputs,
    )
    .map_err(|_| Error::Claims)?;
    let plan = SignedDeltaPlanV3::decode(&packet).map_err(|_| Error::Claims)?;
    let mut ordered_position_accounts =
        Vec::with_capacity(usize::try_from(plan.position_count()).map_err(|_| Error::Claims)?);
    let mut index = 0_u32;
    while index < plan.position_count() {
        let owner =
            Pubkey::new_from_array(plan.position(index).map_err(|_| Error::Claims)?.owner());
        let account = if owner == prepared.root_key() {
            observed.reserve.account
        } else if actor.is_some() && owner.to_bytes() == action.request.input().owner {
            actor.ok_or(Error::Claims)?.account
        } else {
            return Err(Error::Claims);
        };
        ordered_position_accounts.push(account);
        index = index.checked_add(1).ok_or(Error::Claims)?;
    }
    Ok(FractionalSignedDeltaChainPlanV1 {
        lowering,
        packet,
        market_account: observed.market_account,
        ordered_position_accounts,
        expected_post_market,
        expected_post_positions,
        claims_program,
        trading_program,
    })
}

/// Build the exact Claims CPI instruction after validating its canonical FrameSpec.
///
/// Fixed Product/Registry/Core coordinates remain chain-derived by the caller;
/// this function validates exact count, privilege bits, selected programs,
/// caller-authority PDA, Market, and ordered Position tail.
pub fn build_fractional_signed_delta_instruction_v1(
    plan: &FractionalSignedDeltaChainPlanV1,
    accounts: &[AccountMeta],
) -> Result<Instruction> {
    let decoded = SignedDeltaPlanV3::decode(&plan.packet).map_err(|_| Error::Claims)?;
    let spec = SignedDeltaFrameSpecV3::new(decoded.position_count()).map_err(|_| Error::Claims)?;
    if accounts.len() != usize::from(spec.account_count().map_err(|_| Error::Claims)?) {
        return Err(Error::Claims);
    }
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        decoded.release_set(),
        decoded.market(),
        ExecutionRoleV1::Trading,
        decoded.request_id(),
        plan.lowering.packet_digest(),
    )
    .map_err(|_| Error::Claims)?;
    let expected_authority =
        Pubkey::find_program_address(&seeds.as_slices(), &plan.trading_program).0;
    for index in 0..spec.account_count().map_err(|_| Error::Claims)? {
        let expected = spec.account(index).map_err(|_| Error::Claims)?.privileges();
        let observed = accounts.get(usize::from(index)).ok_or(Error::Claims)?;
        if observed.is_signer != expected.signer() || observed.is_writable != expected.writable() {
            return Err(Error::Claims);
        }
    }
    if accounts.first().ok_or(Error::Claims)?.pubkey != expected_authority
        || accounts.get(1).ok_or(Error::Claims)?.pubkey != plan.market_account
        || accounts.get(14).ok_or(Error::Claims)?.pubkey != plan.trading_program
        || accounts.get(16).ok_or(Error::Claims)?.pubkey != plan.claims_program
    {
        return Err(Error::Claims);
    }
    for (index, expected) in plan.ordered_position_accounts.iter().copied().enumerate() {
        let coordinate = usize::from(SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3)
            .checked_add(index)
            .ok_or(Error::Claims)?;
        if accounts.get(coordinate).ok_or(Error::Claims)?.pubkey != expected {
            return Err(Error::Claims);
        }
    }
    Ok(Instruction {
        program_id: plan.claims_program,
        accounts: accounts.to_vec(),
        data: plan.packet.clone(),
    })
}

/// Validate the sole Claims receipt and exact returned post-resource bytes.
pub fn validate_fractional_signed_delta_chain_result_v1(
    plan: &FractionalSignedDeltaChainPlanV1,
    receipt_bytes: &[u8],
    actual_post_market: &[u8],
    actual_post_positions: &[&[u8]],
) -> Result<()> {
    if actual_post_market != plan.expected_post_market
        || actual_post_positions.len() != plan.expected_post_positions.len()
        || actual_post_positions
            .iter()
            .zip(&plan.expected_post_positions)
            .any(|(actual, expected)| *actual != expected.as_slice())
    {
        return Err(Error::Claims);
    }
    validate_fractional_signed_delta_postcondition_v1(
        plan.lowering,
        &plan.packet,
        receipt_bytes,
        actual_post_market,
        actual_post_positions,
    )
    .map_err(|_| Error::Claims)
}

fn authenticate_position(
    claims_program: Pubkey,
    market_account: Pubkey,
    owner: Pubkey,
    position: FractionalClaimsPositionSnapshotV1<'_>,
) -> Result<()> {
    let seeds = ProtocolPositionSeedsV2::new(market_account.to_bytes(), owner.to_bytes())
        .map_err(|_| Error::Claims)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), &claims_program).0;
    if owner == Pubkey::default() || position.account != expected {
        return Err(Error::Claims);
    }
    Ok(())
}
