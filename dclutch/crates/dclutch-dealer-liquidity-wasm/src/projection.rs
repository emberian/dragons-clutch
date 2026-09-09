//! Pure projection from a complete finalized Dealer corpus to one native plan.
//!
//! Discovery owns address traversal. This module owns the sole conversion of
//! that complete corpus into the retained Rust request and Hot instruction
//! constructors, plus the poststate facts exposed to production callers.

use std::str::FromStr;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1,
    hot_v3::{
        DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_MARKET_ACCOUNT_V3,
        HOT_PROGRAM_SET_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    },
    set_v2::CapabilityProgramSetV2,
};
use dclutch_operator::{Finality, Observation, ObservedAccount};
use dclutch_operator::dealer_lp_hot_v4::{
    DealerLpHotStateV4, build_dealer_lp_hot_instruction_v4,
};
use dclutch_operator::direct_inline_v3::{CheckedHotOuterReleaseV3, ObservedAccountMetaV3};
use dclutch_trading::dealer::{Phase, root_tail::RootTail};
use dclutch_trading_sbf::dealer::{
    lp_request::MultiLpChainProjectionV3,
    lp_set_request::{build_close_lp_v4, build_open_lp_v4},
    multi_lp::{
        DEALER_LP_POSITION_BYTES_V3, DEALER_LP_POSITION_PDA_DOMAIN_V3, DealerLpPositionV3,
    },
    obligation::DealerObligationProjectionV3,
};
use serde::Serialize;
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

use crate::wire::{AccountV1, DecimalV1, InputV1, IntentV1, PLAN_FORMAT_V1};

/// Compute budget required by the common Hot transaction.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputeBudgetV1 {
    /// Chain transaction compute ceiling used by canonical operator drivers.
    pub compute_unit_limit: u32,
    /// Runtime heap frame selected by the common Hot ABI.
    pub heap_frame_bytes: u32,
}

/// One canonical instruction account meta.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionAccountV1 {
    /// Base58 account address.
    pub address: String,
    /// Signer privilege.
    pub is_signer: bool,
    /// Writable privilege.
    pub is_writable: bool,
}

/// Exact unsigned instruction reconstructed by native owners.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionV1 {
    /// Selected checked Trading program.
    pub program_id: String,
    /// Ordered physical account metas.
    pub accounts: Vec<InstructionAccountV1>,
    /// Canonical base64 instruction bytes.
    pub data_base64: String,
}

/// Human-readable economic projection, backed by native planner outputs.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewV1 {
    /// Native action name.
    pub action: String,
    /// Canonical LP Position PDA.
    pub lp_position: String,
    /// Canonical Dealer obligation PDA.
    pub obligation: String,
    /// Total pool shares before the action.
    pub shares_before: DecimalV1,
    /// Total pool shares after the action.
    pub shares_after: DecimalV1,
    /// This LP's shares before the action.
    pub lp_shares_before: DecimalV1,
    /// This LP's shares after the action.
    pub lp_shares_after: DecimalV1,
    /// External collateral entering TradingPrincipal.
    pub collateral_in: DecimalV1,
    /// TradingPrincipal collateral returned externally.
    pub collateral_out: DecimalV1,
    /// Actual external token account receiving collateral, when applicable.
    pub recipient: Option<String>,
    /// Claims transferred per outcome.
    pub claims_transferred: Vec<DecimalV1>,
    /// Lifecycle rent destination for Close.
    pub rent_recipient: Option<String>,
    /// Exact lifecycle rent principal returned on Close.
    pub rent_lamports: DecimalV1,
}

/// Complete native plan returned to CLI and browser.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanV1 {
    /// Exact plan transport format.
    pub format: &'static str,
    /// Typed user intent reconstructed by this plan.
    pub intent: IntentV1,
    /// Finalized slot shared by every observation.
    pub observed_slot: DecimalV1,
    /// Canonical base64 family request.
    pub request_base64: String,
    /// Lowercase SHA-256 request digest.
    pub request_digest: String,
    /// Compute budget instructions the caller must prepend in this order.
    pub compute_budget: ComputeBudgetV1,
    /// Sole canonical Trading Hot instruction.
    pub instruction: InstructionV1,
    /// Native economic preview.
    pub preview: PreviewV1,
    /// Exact native-encoded poststates for mutated accounts.
    pub expected_poststates: Vec<AccountV1>,
    /// Authenticated economic accounts required to remain byte-identical.
    pub unchanged_poststates: Vec<AccountV1>,
    /// Named poststates the native planner cannot encode exactly. A nonempty
    /// list prevents production callers from claiming completion.
    pub unverified_poststates: Vec<String>,
    /// Exhaustive account addresses whose finalized states cover every preview
    /// claim and determine whether a durable journal can be cleared.
    pub verification_accounts: Vec<String>,
}

/// Project one complete corpus through native Dealer owners.
pub fn plan(input: &InputV1) -> Result<PlanV1, String> {
    if input.fixed_accounts.is_empty() {
        return Err("Dealer fixed account corpus is empty".to_owned());
    }
    if input.strategy_accounts.is_empty() {
        return Err("Dealer admitted strategy corpus is empty".to_owned());
    }
    match input.intent.action {
        crate::wire::ActionV1::Open | crate::wire::ActionV1::Close => plan_lp(input),
        crate::wire::ActionV1::Add { .. } | crate::wire::ActionV1::Remove { .. } => {
            Err("Dealer equity complete-corpus projection is incomplete".to_owned())
        }
    }
}

fn plan_lp(input: &InputV1) -> Result<PlanV1, String> {
    let observation = observation(input)?;
    let fixed_accounts = convert_accounts(&input.fixed_accounts, observation)?;
    let strategy_accounts = convert_accounts(&input.strategy_accounts, observation)?;
    let runtime_suffix_accounts = convert_accounts(&input.runtime_suffix_accounts, observation)?;
    let market = fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or_else(|| "Dealer fixed corpus omits Market".to_owned())?;
    let root = fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or_else(|| "Dealer fixed corpus omits root".to_owned())?;
    let set_account = fixed_accounts
        .get(HOT_PROGRAM_SET_RAW_ACCOUNT_V3)
        .ok_or_else(|| "Dealer fixed corpus omits ProgramSet".to_owned())?;
    let obligation_account = runtime_suffix_accounts
        .first()
        .ok_or_else(|| "Dealer LP suffix omits obligation".to_owned())?;
    let position_account = runtime_suffix_accounts
        .get(1)
        .ok_or_else(|| "Dealer LP suffix omits LP Position".to_owned())?;
    let obligation = DealerObligationProjectionV3::decode(&obligation_account.account.data)
        .map_err(|error| format!("Dealer obligation: {error:?}"))?;
    let position = if position_account.account.data.is_empty() {
        None
    } else {
        Some(
            DealerLpPositionV3::decode(&position_account.account.data)
                .map_err(|error| format!("Dealer LP Position: {error:?}"))?,
        )
    };
    let root_tail = root
        .account
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or_else(|| "Dealer root omits mutable tail".to_owned())?;
    let phase = RootTail::decode(root_tail)
        .map_err(|error| format!("Dealer root tail: {error:?}"))?
        .phase;
    let owner = parse_key(&input.intent.owner, "LP owner")?;
    let trading = parse_key(&input.programs.trading, "Trading program")?;
    let expected_position = Pubkey::find_program_address(
        &[
            DEALER_LP_POSITION_PDA_DOMAIN_V3,
            root.account.key.as_ref(),
            owner.as_ref(),
        ],
        &trading,
    )
    .0;
    if position_account.account.key != expected_position {
        return Err("Dealer LP Position differs from canonical PDA".to_owned());
    }
    let chain = MultiLpChainProjectionV3 {
        trading_program: trading.to_bytes(),
        release_set: decode_hex_32(&input.checked_release.release_set, "release set")?,
        market: market.account.key.to_bytes(),
        child_root: root.account.key.to_bytes(),
        lp_position_address: expected_position.to_bytes(),
        lp_position: position,
        lp_position_bytes: position.map(|_| position_account.account.data.as_slice()),
        obligation,
        obligation_address: obligation_account.account.key.to_bytes(),
        generation: input.checked_release.generation.0,
        now: input.observation.slot.0,
        expires_at: input.intent.expires_at.0,
        lp_position_rent_principal: input.lp_position_rent_lamports.0,
        terminal: phase != Phase::Open,
    };
    let set = CapabilityProgramSetV2::decode(&set_account.account.data)
        .map_err(|error| format!("Dealer ProgramSet: {error:?}"))?;
    let unsigned = match input.intent.action {
        crate::wire::ActionV1::Open => build_open_lp_v4(chain, owner.to_bytes(), set),
        crate::wire::ActionV1::Close => build_close_lp_v4(chain, set),
        _ => unreachable!("LP projection action"),
    }
    .map_err(|error| format!("Dealer LP request: {error:?}"))?;
    let request = unsigned.as_bytes();
    let state = DealerLpHotStateV4 {
        fixed_accounts: fixed_accounts.clone(),
        strategy_accounts: strategy_accounts.clone(),
        runtime_suffix_accounts: runtime_suffix_accounts.clone(),
        release_set: chain.release_set,
        generation: chain.generation,
        hot_outer: Some(CheckedHotOuterReleaseV3 {
            trading_program: trading,
            artifact_release: decode_hex_32(
                &input.checked_release.trading_artifact_release,
                "Trading artifact release",
            )?,
            checked_manifest_digest: decode_hex_32(
                &input.checked_release.checked_manifest_digest,
                "checked manifest digest",
            )?,
        }),
    };
    let report = build_dealer_lp_hot_instruction_v4(&state, request)
        .map_err(|error| format!("Dealer LP Hot: {error:?}"))?;

    let shares = DecimalV1(obligation.total_equity_shares());
    let mut expected_poststates = Vec::new();
    let mut unchanged_poststates = vec![
        input.fixed_accounts[HOT_ROOT_ACCOUNT_V3].clone(),
        input.runtime_suffix_accounts[0].clone(),
    ];
    let (action, rent_recipient, rent_lamports) = match input.intent.action {
        crate::wire::ActionV1::Open => {
            let (_, bump) = Pubkey::find_program_address(
                &[
                    DEALER_LP_POSITION_PDA_DOMAIN_V3,
                    root.account.key.as_ref(),
                    owner.as_ref(),
                ],
                &trading,
            );
            let expected = DealerLpPositionV3 {
                revision: 1,
                release_set: chain.release_set,
                market: market.account.key.to_bytes(),
                child_root: root.account.key.to_bytes(),
                lp_owner: owner.to_bytes(),
                rent_refund: owner.to_bytes(),
                obligation_account: obligation_account.account.key.to_bytes(),
                equity_shares: 0,
                generation: chain.generation,
                rent_principal: chain.lp_position_rent_principal,
                pda_bump: u16::from(bump),
            };
            let mut data = vec![0_u8; DEALER_LP_POSITION_BYTES_V3];
            expected
                .encode_into(&mut data)
                .map_err(|error| format!("Dealer LP open poststate: {error:?}"))?;
            expected_poststates.push(account_poststate(
                &input.runtime_suffix_accounts[1],
                &trading,
                chain.lp_position_rent_principal,
                data,
            ));
            // Open leaves the Market LifecycleRentCredit byte-for-byte intact.
            unchanged_poststates.push(
                input.runtime_suffix_accounts
                    .get(3)
                    .ok_or_else(|| "Dealer Open suffix omits LifecycleRentCredit".to_owned())?
                    .clone(),
            );
            ("open".to_owned(), None, chain.lp_position_rent_principal)
        }
        crate::wire::ActionV1::Close => {
            let credit = input
                .runtime_suffix_accounts
                .get(2)
                .ok_or_else(|| "Dealer Close suffix omits LifecycleRentCredit".to_owned())?;
            expected_poststates.push(account_poststate(
                &input.runtime_suffix_accounts[1],
                &system_program::ID,
                0,
                Vec::new(),
            ));
            expected_poststates.push(account_poststate(
                credit,
                &parse_key(&credit.owner, "LifecycleRentCredit owner")?,
                credit
                    .lamports
                    .0
                    .checked_add(position_account.account.lamports)
                    .ok_or_else(|| "Dealer close rent credit overflow".to_owned())?,
                STANDARD
                    .decode(&credit.data_base64)
                    .map_err(|_| "Dealer LifecycleRentCredit base64 refused".to_owned())?,
            ));
            (
                "close".to_owned(),
                Some(credit.address.clone()),
                chain.lp_position_rent_principal,
            )
        }
        _ => unreachable!("LP projection action"),
    };
    let verification_accounts = expected_poststates
        .iter()
        .chain(unchanged_poststates.iter())
        .map(|account| account.address.clone())
        .collect();
    let request_digest = hash(request).to_bytes();
    Ok(PlanV1 {
        format: PLAN_FORMAT_V1,
        intent: input.intent.clone(),
        observed_slot: input.observation.slot.clone(),
        request_base64: STANDARD.encode(request),
        request_digest: encode_hex(request_digest),
        compute_budget: ComputeBudgetV1 {
            compute_unit_limit: dclutch_operator::registry::TRANSACTION_COMPUTE_UNIT_LIMIT_V1,
            heap_frame_bytes: DIRECT_HOT_HEAP_FRAME_BYTES_V1,
        },
        instruction: InstructionV1 {
            program_id: report.instruction.program_id.to_string(),
            accounts: report
                .instruction
                .accounts
                .iter()
                .map(|meta| InstructionAccountV1 {
                    address: meta.pubkey.to_string(),
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                })
                .collect(),
            data_base64: STANDARD.encode(&report.instruction.data),
        },
        preview: PreviewV1 {
            action,
            lp_position: expected_position.to_string(),
            obligation: obligation_account.account.key.to_string(),
            shares_before: shares.clone(),
            shares_after: shares,
            lp_shares_before: DecimalV1(position.map_or(0, |value| value.equity_shares)),
            lp_shares_after: DecimalV1(0),
            collateral_in: DecimalV1(0),
            collateral_out: DecimalV1(0),
            recipient: None,
            claims_transferred: Vec::new(),
            rent_recipient,
            rent_lamports: DecimalV1(rent_lamports),
        },
        expected_poststates,
        unchanged_poststates,
        unverified_poststates: Vec::new(),
        verification_accounts,
    })
}

fn observation(input: &InputV1) -> Result<Observation, String> {
    Ok(Observation {
        slot: input.observation.slot.0,
        unix_timestamp: input
            .observation
            .unix_timestamp
            .parse::<i64>()
            .map_err(|_| "Dealer observation unixTimestamp refused".to_owned())?,
        finality: Finality::Finalized,
    })
}

fn convert_accounts(
    values: &[AccountV1],
    observation: Observation,
) -> Result<Vec<ObservedAccountMetaV3>, String> {
    values
        .iter()
        .map(|value| {
            Ok(ObservedAccountMetaV3 {
                account: ObservedAccount {
                    observation,
                    key: parse_key(&value.address, "account address")?,
                    owner: parse_key(&value.owner, "account owner")?,
                    lamports: value.lamports.0,
                    executable: value.executable,
                    data: STANDARD
                        .decode(&value.data_base64)
                        .map_err(|_| "Dealer account base64 refused".to_owned())?,
                },
                is_signer: value.is_signer,
                is_writable: value.is_writable,
            })
        })
        .collect()
}

fn parse_key(value: &str, label: &str) -> Result<Pubkey, String> {
    Pubkey::from_str(value).map_err(|_| format!("Dealer {label} base58 refused"))
}

fn decode_hex_32(value: &str, label: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(format!("Dealer {label} must be lowercase SHA-256 hex"));
    }
    let mut output = [0_u8; 32];
    for (index, destination) in output.iter_mut().enumerate() {
        *destination = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| format!("Dealer {label} hex refused"))?;
    }
    if output == [0; 32] {
        return Err(format!("Dealer {label} is zero"));
    }
    Ok(output)
}

fn encode_hex(value: [u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in value {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("String formatting cannot fail");
    }
    output
}

fn account_poststate(template: &AccountV1, owner: &Pubkey, lamports: u64, data: Vec<u8>) -> AccountV1 {
    AccountV1 {
        address: template.address.clone(),
        owner: owner.to_string(),
        lamports: DecimalV1(lamports),
        executable: false,
        data_base64: STANDARD.encode(data),
        is_signer: template.is_signer,
        is_writable: template.is_writable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_format_and_budget_are_protocol_owned() {
        assert_eq!(PLAN_FORMAT_V1, "dclutch-dealer-liquidity-plan-v1");
        assert_eq!(dclutch_operator::registry::TRANSACTION_COMPUTE_UNIT_LIMIT_V1, 1_400_000);
        assert!(
            dclutch_market::capability_program::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1 >= 32 * 1024
        );
    }
}
