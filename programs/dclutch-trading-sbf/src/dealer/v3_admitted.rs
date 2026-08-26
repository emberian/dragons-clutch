//! Read-only admitted-AOT evaluation for Dealer scenario exact-fill.
//!
//! The accelerator evaluates the same canonical scenario, Claims, and Custody
//! composition used by the host constructor, but returns only a candidate
//! register bank. It owns no account, invokes no child, and cannot commit the
//! Trading obligation. Common Hot remains the sole Effect executor and writer.

use solana_program::hash::hash;

use super::{
    v3_composer::{
        MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3, ScenarioAtomicPlanV3, ScenarioCollateralFrameV3,
        ScenarioComposerContextV3, ScenarioCustodyEffectV3,
    },
    v3_trade::{
        DealerScenarioTradeRequestV3, ScenarioTradeChainProjectionV3, prepare_scenario_trade_v3,
    },
    v3_trade_artifacts::{
        DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4, DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
        DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4, DEALER_SCENARIO_EXPIRY_SCALAR_V4,
        DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4, DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4,
        project_dealer_scenario_hot_registers_v4,
    },
};

/// Stable refusal from the read-only Dealer admitted evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioAdmittedErrorV4 {
    /// The exact family request or authenticated pre-transition bank refused.
    Input,
    /// Runtime-width semantic scratch did not match Product N.
    Geometry,
    /// The canonical scenario/Claims/Custody composition refused.
    Semantics,
    /// Candidate register projection or scalar-then-identity encoding refused.
    Candidate,
    /// Checked byte or coordinate arithmetic overflowed.
    Arithmetic,
}

/// Caller-owned runtime-width scratch and candidate storage.
///
/// Semantic scratch may be modified on refusal. `candidate_bank` is commit-last
/// and remains byte-for-byte unchanged unless the complete evaluation accepts.
pub struct DealerScenarioAdmittedBuffersV4<'a> {
    /// Request-derived claims acquired by Dealer.
    pub acquired: &'a mut [u64],
    /// Request-derived claims delivered by Dealer.
    pub delivered: &'a mut [u64],
    /// Current obligation vector scratch.
    pub obligations_before: &'a mut [u64],
    /// Candidate obligation vector scratch.
    pub obligations_after: &'a mut [u64],
    /// Candidate Dealer Claims inventory.
    pub post_inventory: &'a mut [u64],
    /// Candidate counterparty Claims inventory.
    pub post_counterparty_inventory: &'a mut [u64],
    /// Candidate scenario residual vector.
    pub post_equity: &'a mut [i128],
    /// Exact ordered active Custody requests produced by the semantic composer.
    pub custody_effects:
        &'a mut [Option<ScenarioCustodyEffectV3>; MAX_DEALER_SCENARIO_CUSTODY_EFFECTS_V3],
    /// Candidate scalar-bank scratch, exact width `97 + N`.
    pub candidate_scalars: &'a mut [u64],
    /// Candidate identity-bank scratch, exact width 115.
    pub candidate_identities: &'a mut [[u8; 32]],
    /// Non-authoritative byte encoding scratch.
    pub bank_scratch: &'a mut [u8],
    /// Complete accepted scalar-then-identity bank, written last.
    pub candidate_bank: &'a mut [u8],
}

/// Evaluate selector 9 into one complete authoritative candidate bank.
///
/// `input_bank` is the exact Account/Request/Lifecycle projection authenticated
/// by common Hot and transported in `AcceleratorRequestV2`. This evaluator
/// rechecks every request/environment coordinate it consumes before deriving
/// the scenario plan. The exact SignedDelta packet remains the sole owner of
/// Claims rows; no acquired/delivered vector is accepted from the caller.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_dealer_scenario_admitted_v4(
    family_request: &[u8],
    input_bank: &[u8],
    chain: ScenarioTradeChainProjectionV3<'_>,
    context: ScenarioComposerContextV3,
    collateral: ScenarioCollateralFrameV3,
    trusted_current_slot: u64,
    buffers: DealerScenarioAdmittedBuffersV4<'_>,
) -> Result<ScenarioAtomicPlanV3, DealerScenarioAdmittedErrorV4> {
    let request = DealerScenarioTradeRequestV3::decode(family_request)
        .map_err(|_| DealerScenarioAdmittedErrorV4::Input)?;
    let width =
        usize::try_from(request.width).map_err(|_| DealerScenarioAdmittedErrorV4::Arithmetic)?;
    let scalar_count = usize::from(DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4)
        .checked_add(width)
        .ok_or(DealerScenarioAdmittedErrorV4::Arithmetic)?;
    let identity_count = usize::from(DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4);
    let bank_bytes = register_bank_bytes(scalar_count, identity_count)?;
    let DealerScenarioAdmittedBuffersV4 {
        acquired,
        delivered,
        obligations_before,
        obligations_after,
        post_inventory,
        post_counterparty_inventory,
        post_equity,
        custody_effects,
        candidate_scalars,
        candidate_identities,
        bank_scratch,
        candidate_bank,
    } = buffers;
    if trusted_current_slot != chain.now
        || [
            acquired.len(),
            delivered.len(),
            obligations_before.len(),
            obligations_after.len(),
            post_inventory.len(),
            post_counterparty_inventory.len(),
            post_equity.len(),
        ]
        .iter()
        .any(|observed| *observed != width)
        || candidate_scalars.len() != scalar_count
        || candidate_identities.len() != identity_count
        || input_bank.len() != bank_bytes
        || bank_scratch.len() != bank_bytes
        || candidate_bank.len() != bank_bytes
    {
        return Err(DealerScenarioAdmittedErrorV4::Geometry);
    }
    authenticate_input_bank(input_bank, scalar_count, request, trusted_current_slot)?;

    let plan = prepare_scenario_trade_v3(
        request,
        chain,
        context,
        collateral,
        acquired,
        delivered,
        obligations_before,
        obligations_after,
        post_inventory,
        post_counterparty_inventory,
        post_equity,
        custody_effects,
    )
    .map_err(|_| DealerScenarioAdmittedErrorV4::Semantics)?;
    project_dealer_scenario_hot_registers_v4(
        request,
        plan,
        chain.candidate_obligation,
        custody_effects,
        trusted_current_slot,
        candidate_scalars,
        candidate_identities,
    )
    .map_err(|_| DealerScenarioAdmittedErrorV4::Candidate)?;
    encode_register_bank(candidate_scalars, candidate_identities, bank_scratch)?;
    candidate_bank.copy_from_slice(bank_scratch);
    Ok(plan)
}

fn authenticate_input_bank(
    input_bank: &[u8],
    scalar_count: usize,
    request: DealerScenarioTradeRequestV3<'_>,
    trusted_current_slot: u64,
) -> Result<(), DealerScenarioAdmittedErrorV4> {
    for (index, expected) in [
        (
            DEALER_SCENARIO_POSITION_COUNT_SCALAR_V4,
            u64::from(request.claims_position_count),
        ),
        (
            DEALER_SCENARIO_WITNESS_BYTES_SCALAR_V4,
            u64::from(request.claims_packet_bytes),
        ),
        (DEALER_SCENARIO_CURRENT_SLOT_SCALAR_V4, trusted_current_slot),
        (DEALER_SCENARIO_EXPIRY_SCALAR_V4, request.expires_at),
    ] {
        if read_scalar(input_bank, index)? != expected {
            return Err(DealerScenarioAdmittedErrorV4::Input);
        }
    }
    if read_identity(input_bank, scalar_count, 0)? != hash(request.bytes()).to_bytes() {
        return Err(DealerScenarioAdmittedErrorV4::Input);
    }
    Ok(())
}

fn register_bank_bytes(
    scalar_count: usize,
    identity_count: usize,
) -> Result<usize, DealerScenarioAdmittedErrorV4> {
    scalar_count
        .checked_mul(8)
        .and_then(|bytes| {
            identity_count
                .checked_mul(32)
                .and_then(|ids| bytes.checked_add(ids))
        })
        .ok_or(DealerScenarioAdmittedErrorV4::Arithmetic)
}

fn read_scalar(bank: &[u8], index: u16) -> Result<u64, DealerScenarioAdmittedErrorV4> {
    let offset = usize::from(index)
        .checked_mul(8)
        .ok_or(DealerScenarioAdmittedErrorV4::Arithmetic)?;
    let bytes = bank
        .get(offset..offset + 8)
        .ok_or(DealerScenarioAdmittedErrorV4::Input)?;
    Ok(u64::from_le_bytes(
        bytes
            .try_into()
            .map_err(|_| DealerScenarioAdmittedErrorV4::Input)?,
    ))
}

fn read_identity(
    bank: &[u8],
    scalar_count: usize,
    index: usize,
) -> Result<[u8; 32], DealerScenarioAdmittedErrorV4> {
    let offset = scalar_count
        .checked_mul(8)
        .and_then(|base| {
            index
                .checked_mul(32)
                .and_then(|item| base.checked_add(item))
        })
        .ok_or(DealerScenarioAdmittedErrorV4::Arithmetic)?;
    bank.get(offset..offset + 32)
        .ok_or(DealerScenarioAdmittedErrorV4::Input)?
        .try_into()
        .map_err(|_| DealerScenarioAdmittedErrorV4::Input)
}

fn encode_register_bank(
    scalars: &[u64],
    identities: &[[u8; 32]],
    output: &mut [u8],
) -> Result<(), DealerScenarioAdmittedErrorV4> {
    if output.len() != register_bank_bytes(scalars.len(), identities.len())? {
        return Err(DealerScenarioAdmittedErrorV4::Candidate);
    }
    let mut cursor = 0_usize;
    for value in scalars {
        let end = cursor
            .checked_add(8)
            .ok_or(DealerScenarioAdmittedErrorV4::Arithmetic)?;
        output
            .get_mut(cursor..end)
            .ok_or(DealerScenarioAdmittedErrorV4::Candidate)?
            .copy_from_slice(&value.to_le_bytes());
        cursor = end;
    }
    for value in identities {
        let end = cursor
            .checked_add(32)
            .ok_or(DealerScenarioAdmittedErrorV4::Arithmetic)?;
        output
            .get_mut(cursor..end)
            .ok_or(DealerScenarioAdmittedErrorV4::Candidate)?
            .copy_from_slice(value);
        cursor = end;
    }
    if cursor != output.len() {
        return Err(DealerScenarioAdmittedErrorV4::Candidate);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_then_identity_bank_is_exact_and_checked() {
        let scalars = [0_u64, 1, u64::MAX];
        let identities = [[7_u8; 32], [8_u8; 32]];
        let mut bank = [0_u8; 88];
        encode_register_bank(&scalars, &identities, &mut bank).expect("encode");
        assert_eq!(read_scalar(&bank, 0), Ok(0));
        assert_eq!(read_scalar(&bank, 1), Ok(1));
        assert_eq!(read_scalar(&bank, 2), Ok(u64::MAX));
        assert_eq!(read_identity(&bank, 3, 0), Ok([7; 32]));
        assert_eq!(read_identity(&bank, 3, 1), Ok([8; 32]));
        assert_eq!(
            encode_register_bank(&scalars, &identities, &mut bank[..87]),
            Err(DealerScenarioAdmittedErrorV4::Candidate)
        );
    }
}
