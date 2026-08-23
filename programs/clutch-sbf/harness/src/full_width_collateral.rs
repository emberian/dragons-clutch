//! Canonical host transaction builders for the full-width collateral routes.
//!
//! These builders do not provision state and do not reinterpret any lowered
//! fixture address. Callers must supply exact Product, General, collateral,
//! and claim-plane accounts produced by the live founding flow.

use super::{budget_instruction, transaction, Instruction, Message, COMPUTE_UNIT_CEILING};
use clutch_sbf::instructions::{
    claim_representation_v3, collateral_cash_v3, complete_set_v3, external_redemption_v3,
};
use clutch_solana_layout::collateral_v3_accounts::{
    account_contract_v3, validate_collateral_account_metas_v3, CollateralAccountContractV3,
    CollateralActionV3, ObservedCollateralAccountMetaV3,
};
use clutch_solana_layout::Intent;
use clutch_solana_reference::{Action, Request};

/// Shared authenticated accounts for one full-width MarketInstanceV2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FullWidthCollateralMarketV3 {
    pub realm: [u8; 32],
    pub profile: [u8; 32],
    pub collateral_policy: [u8; 32],
    pub collateral_token_program: [u8; 32],
    pub market_binding: [u8; 32],
    pub market_runtime: [u8; 32],
    pub market_instance_artifact: [u8; 32],
    pub hoard: [u8; 32],
    pub claim_ledger: [u8; 32],
    pub collateral_mint: [u8; 32],
    pub hoard_authority: [u8; 32],
    pub hoard_token: [u8; 32],
    pub outcome_token_program: [u8; 32],
    pub outcome_token_programdata: [u8; 32],
    pub outcome_mints: Vec<[u8; 32]>,
}

impl FullWidthCollateralMarketV3 {
    fn require_outcome(&self, outcome: u8) -> usize {
        let selected = usize::from(outcome);
        assert!(
            !self.outcome_mints.is_empty()
                && self.outcome_mints.len() <= clutch_solana_layout::MAX_OUTCOMES
                && selected < self.outcome_mints.len(),
            "full-width collateral builder requires one canonical mint per active outcome"
        );
        for (index, mint) in self.outcome_mints.iter().enumerate() {
            assert!(
                !self.outcome_mints[index + 1..].contains(mint),
                "full-width collateral builder requires distinct outcome mints"
            );
        }
        selected
    }
}

/// Exact ordinary PositionV3/GEN1 owner plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FullWidthCollateralOwnerV3 {
    pub owner: [u8; 32],
    pub position: [u8; 32],
    pub replay: [u8; 32],
    pub collateral_token: [u8; 32],
}

fn push_unique(values: &mut Vec<[u8; 32]>, value: [u8; 32]) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn claim_readonly_roles(
    market: &FullWidthCollateralMarketV3,
    selected: usize,
    fixed: &[[u8; 32]],
) -> Vec<[u8; 32]> {
    let mut readonly = fixed.to_vec();
    for (index, mint) in market.outcome_mints.iter().enumerate() {
        if index != selected {
            readonly.push(*mint);
        }
    }
    readonly
}

fn contract_from_request(data: &[u8], outcome_mints: usize) -> CollateralAccountContractV3 {
    let request = Request::decode(data).expect("collateral builder requires one exact request");
    let (action, selected_outcome) = match request.action {
        Action::Layout(Intent::Endow { .. }) => (CollateralActionV3::Endow, None),
        Action::Layout(Intent::WithdrawCash { .. }) => (CollateralActionV3::WithdrawCash, None),
        Action::Layout(Intent::Split { .. }) => (CollateralActionV3::Split, None),
        Action::Layout(Intent::Merge { .. }) => (CollateralActionV3::Merge, None),
        Action::Layout(Intent::Materialize { outcome, .. }) => {
            (CollateralActionV3::Materialize, Some(outcome))
        }
        Action::Layout(Intent::Dematerialize { outcome, .. }) => {
            (CollateralActionV3::Dematerialize, Some(outcome))
        }
        Action::Layout(Intent::RedeemExternal { outcome, .. }) if request.sequence == 0 => {
            (CollateralActionV3::RedeemExternal, Some(outcome))
        }
        _ => panic!("request is not an enabled full-width collateral route"),
    };
    account_contract_v3(
        action,
        u8::try_from(outcome_mints).expect("active market width must fit u8"),
        selected_outcome,
    )
    .expect("active market width and selected outcome must be canonical")
}

fn build(
    fee_payer: [u8; 32],
    program: [u8; 32],
    compute_budget: [u8; 32],
    writable_signers: &[[u8; 32]],
    readonly_signers: &[[u8; 32]],
    writable: &[[u8; 32]],
    readonly_roles: &[[u8; 32]],
    contract: CollateralAccountContractV3,
    role_keys: &[[u8; 32]],
    data: Vec<u8>,
) -> Vec<u8> {
    let mut writable_signer_keys = vec![fee_payer];
    for key in writable_signers {
        push_unique(&mut writable_signer_keys, *key);
    }
    let mut readonly_signer_keys = Vec::new();
    for key in readonly_signers {
        if !writable_signer_keys.contains(key) {
            push_unique(&mut readonly_signer_keys, *key);
        }
    }
    let mut writable_keys = Vec::new();
    for key in writable {
        if !writable_signer_keys.contains(key) && !readonly_signer_keys.contains(key) {
            push_unique(&mut writable_keys, *key);
        }
    }
    let mut readonly_keys = Vec::new();
    for key in readonly_roles
        .iter()
        .chain([program, compute_budget].iter())
    {
        if !writable_signer_keys.contains(key)
            && !readonly_signer_keys.contains(key)
            && !writable_keys.contains(key)
        {
            push_unique(&mut readonly_keys, *key);
        }
    }
    let message = Message::new(
        &writable_signer_keys,
        &readonly_signer_keys,
        &writable_keys,
        &readonly_keys,
    );
    let observed = role_keys
        .iter()
        .map(|key| {
            let index = usize::from(message.index(key));
            let required_signatures = usize::from(message.required_signatures);
            let writable_signatures = required_signatures - usize::from(message.readonly_signed);
            let writable_unsigned_end = message.keys.len() - usize::from(message.readonly_unsigned);
            ObservedCollateralAccountMetaV3 {
                key: *key,
                signer: index < required_signatures,
                writable: index < writable_signatures
                    || (index >= required_signatures && index < writable_unsigned_end),
            }
        })
        .collect::<Vec<_>>();
    validate_collateral_account_metas_v3(
        contract.action(),
        contract.outcome_count(),
        contract.selected_outcome(),
        &observed,
    )
    .expect("full-width collateral message must preserve the canonical V3 account contract");
    let action = Instruction {
        program_index: message.index(&program),
        accounts: message.indices(role_keys),
        data,
    };
    transaction(
        &message,
        &[
            budget_instruction(&message, &compute_budget, COMPUTE_UNIT_CEILING),
            action,
        ],
    )
}

/// Build canonical full-width Endow, including first-owner creation roles.
pub fn endow_v3_transaction(
    fee_payer: [u8; 32],
    program: [u8; 32],
    compute_budget: [u8; 32],
    system_program: [u8; 32],
    rent_sysvar: [u8; 32],
    market: &FullWidthCollateralMarketV3,
    owner: FullWidthCollateralOwnerV3,
    data: Vec<u8>,
) -> Vec<u8> {
    let roles = [
        owner.owner,
        market.realm,
        market.profile,
        market.collateral_policy,
        market.collateral_token_program,
        market.market_binding,
        market.market_runtime,
        market.market_instance_artifact,
        market.hoard,
        market.claim_ledger,
        owner.position,
        owner.replay,
        market.collateral_mint,
        owner.collateral_token,
        market.hoard_authority,
        market.hoard_token,
        system_program,
        rent_sysvar,
    ];
    assert_eq!(roles.len(), collateral_cash_v3::ENDOW_ACCOUNT_COUNT_V3);
    let contract = contract_from_request(&data, market.outcome_mints.len());
    assert_eq!(contract.action(), CollateralActionV3::Endow);
    build(
        fee_payer,
        program,
        compute_budget,
        &[owner.owner],
        &[],
        &[
            market.hoard,
            owner.position,
            owner.replay,
            owner.collateral_token,
            market.hoard_token,
        ],
        &[
            market.realm,
            market.profile,
            market.collateral_policy,
            market.collateral_token_program,
            market.market_binding,
            market.market_runtime,
            market.market_instance_artifact,
            market.claim_ledger,
            market.collateral_mint,
            market.hoard_authority,
            system_program,
            rent_sysvar,
        ],
        contract,
        &roles,
        data,
    )
}

/// Build canonical full-width owner cash withdrawal.
pub fn withdraw_cash_v3_transaction(
    fee_payer: [u8; 32],
    program: [u8; 32],
    compute_budget: [u8; 32],
    market: &FullWidthCollateralMarketV3,
    owner: FullWidthCollateralOwnerV3,
    destination: [u8; 32],
    data: Vec<u8>,
) -> Vec<u8> {
    let roles = [
        owner.owner,
        market.realm,
        market.profile,
        market.collateral_policy,
        market.collateral_token_program,
        market.market_binding,
        market.market_runtime,
        market.market_instance_artifact,
        market.hoard,
        market.claim_ledger,
        owner.position,
        owner.replay,
        market.collateral_mint,
        destination,
        market.hoard_authority,
        market.hoard_token,
    ];
    assert_eq!(roles.len(), collateral_cash_v3::WITHDRAW_ACCOUNT_COUNT_V3);
    let contract = contract_from_request(&data, market.outcome_mints.len());
    assert_eq!(contract.action(), CollateralActionV3::WithdrawCash);
    build(
        fee_payer,
        program,
        compute_budget,
        &[],
        &[owner.owner],
        &[
            market.hoard,
            owner.position,
            owner.replay,
            destination,
            market.hoard_token,
        ],
        &[
            market.realm,
            market.profile,
            market.collateral_policy,
            market.collateral_token_program,
            market.market_binding,
            market.market_runtime,
            market.market_instance_artifact,
            market.claim_ledger,
            market.collateral_mint,
            market.hoard_authority,
        ],
        contract,
        &roles,
        data,
    )
}

/// Build canonical full-width Split or Merge; the request bytes select which.
pub fn complete_set_v3_transaction(
    fee_payer: [u8; 32],
    program: [u8; 32],
    compute_budget: [u8; 32],
    market: &FullWidthCollateralMarketV3,
    owner: FullWidthCollateralOwnerV3,
    data: Vec<u8>,
) -> Vec<u8> {
    let roles = [
        owner.owner,
        market.realm,
        market.profile,
        market.collateral_policy,
        market.collateral_token_program,
        market.market_binding,
        market.market_runtime,
        market.market_instance_artifact,
        market.hoard,
        market.claim_ledger,
        owner.position,
        owner.replay,
        market.collateral_mint,
        market.hoard_token,
    ];
    assert_eq!(roles.len(), complete_set_v3::COMPLETE_SET_ACCOUNT_COUNT_V3);
    let contract = contract_from_request(&data, market.outcome_mints.len());
    assert!(matches!(
        contract.action(),
        CollateralActionV3::Split | CollateralActionV3::Merge
    ));
    build(
        fee_payer,
        program,
        compute_budget,
        &[],
        &[owner.owner],
        &[
            market.hoard,
            market.claim_ledger,
            owner.position,
            owner.replay,
        ],
        &[
            market.realm,
            market.profile,
            market.collateral_policy,
            market.collateral_token_program,
            market.market_binding,
            market.market_runtime,
            market.market_instance_artifact,
            market.collateral_mint,
            market.hoard_token,
        ],
        contract,
        &roles,
        data,
    )
}

/// Build canonical full-width Materialize or Dematerialize; request bytes
/// select the action and must name the supplied holder token and outcome.
pub fn claim_representation_v3_transaction(
    fee_payer: [u8; 32],
    program: [u8; 32],
    compute_budget: [u8; 32],
    market: &FullWidthCollateralMarketV3,
    owner: FullWidthCollateralOwnerV3,
    holder_token: [u8; 32],
    outcome: u8,
    data: Vec<u8>,
) -> Vec<u8> {
    let selected = market.require_outcome(outcome);
    let mut roles = vec![
        owner.owner,
        market.realm,
        market.profile,
        market.collateral_policy,
        market.collateral_token_program,
        market.market_binding,
        market.market_runtime,
        market.market_instance_artifact,
        market.hoard,
        market.claim_ledger,
        owner.position,
        owner.replay,
        market.outcome_token_program,
        holder_token,
        market.outcome_token_programdata,
    ];
    roles.extend_from_slice(&market.outcome_mints);
    assert_eq!(
        roles.len(),
        claim_representation_v3::CLAIM_REPRESENTATION_PREFIX_ACCOUNTS_V3
            + market.outcome_mints.len()
    );
    let readonly = claim_readonly_roles(
        market,
        selected,
        &[
            market.realm,
            market.profile,
            market.collateral_policy,
            market.collateral_token_program,
            market.market_binding,
            market.market_runtime,
            market.market_instance_artifact,
            market.hoard,
            market.outcome_token_program,
            market.outcome_token_programdata,
        ],
    );
    let contract = contract_from_request(&data, market.outcome_mints.len());
    assert!(matches!(
        contract.action(),
        CollateralActionV3::Materialize | CollateralActionV3::Dematerialize
    ));
    assert_eq!(contract.selected_outcome(), Some(outcome));
    build(
        fee_payer,
        program,
        compute_budget,
        &[],
        &[owner.owner],
        &[
            market.claim_ledger,
            owner.position,
            owner.replay,
            holder_token,
            market.outcome_mints[selected],
        ],
        &readonly,
        contract,
        &roles,
        data,
    )
}

/// Build canonical exact-whole external bearer redemption over ResolutionV5.
pub fn redeem_external_v3_transaction(
    fee_payer: [u8; 32],
    program: [u8; 32],
    compute_budget: [u8; 32],
    market: &FullWidthCollateralMarketV3,
    claimant: [u8; 32],
    source: [u8; 32],
    destination: [u8; 32],
    resolution_v5: [u8; 32],
    outcome: u8,
    data: Vec<u8>,
) -> Vec<u8> {
    assert_ne!(
        fee_payer, claimant,
        "external claimant must remain read-only"
    );
    let selected = market.require_outcome(outcome);
    let mut roles = vec![
        claimant,
        market.realm,
        market.profile,
        market.collateral_policy,
        market.collateral_token_program,
        market.market_binding,
        market.market_runtime,
        market.market_instance_artifact,
        market.hoard,
        market.claim_ledger,
        resolution_v5,
        market.collateral_mint,
        destination,
        market.hoard_authority,
        market.hoard_token,
        market.outcome_token_program,
        source,
        market.outcome_token_programdata,
    ];
    roles.extend_from_slice(&market.outcome_mints);
    assert_eq!(
        roles.len(),
        external_redemption_v3::EXTERNAL_REDEMPTION_PREFIX_ACCOUNTS_V3 + market.outcome_mints.len()
    );
    let readonly = claim_readonly_roles(
        market,
        selected,
        &[
            market.realm,
            market.profile,
            market.collateral_policy,
            market.collateral_token_program,
            market.market_binding,
            market.market_runtime,
            market.market_instance_artifact,
            resolution_v5,
            market.collateral_mint,
            market.hoard_authority,
            market.outcome_token_program,
            market.outcome_token_programdata,
        ],
    );
    let contract = contract_from_request(&data, market.outcome_mints.len());
    assert_eq!(contract.action(), CollateralActionV3::RedeemExternal);
    assert_eq!(contract.selected_outcome(), Some(outcome));
    build(
        fee_payer,
        program,
        compute_budget,
        &[],
        &[claimant],
        &[
            market.hoard,
            market.claim_ledger,
            destination,
            market.hoard_token,
            source,
            market.outcome_mints[selected],
        ],
        &readonly,
        contract,
        &roles,
        data,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clutch_solana_layout::Hash32;

    fn key(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn market() -> FullWidthCollateralMarketV3 {
        FullWidthCollateralMarketV3 {
            realm: key(10),
            profile: key(11),
            collateral_policy: key(12),
            collateral_token_program: key(13),
            market_binding: key(14),
            market_runtime: key(15),
            market_instance_artifact: key(16),
            hoard: key(17),
            claim_ledger: key(18),
            collateral_mint: key(19),
            hoard_authority: key(20),
            hoard_token: key(21),
            outcome_token_program: key(22),
            outcome_token_programdata: key(25),
            outcome_mints: vec![key(23), key(24)],
        }
    }

    fn owner() -> FullWidthCollateralOwnerV3 {
        FullWidthCollateralOwnerV3 {
            owner: key(30),
            position: key(31),
            replay: key(32),
            collateral_token: key(33),
        }
    }

    fn request(sequence: u64, intent: Intent) -> Vec<u8> {
        let mut encoded_intent = vec![0; intent.encoded_len()];
        let written = intent.encode(&mut encoded_intent).unwrap();
        encoded_intent.truncate(written);
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&sequence.to_le_bytes());
        data.push(0);
        let encoded_intent_len =
            u16::try_from(encoded_intent.len()).expect("fixed Request intent fits u16");
        data.extend_from_slice(&encoded_intent_len.to_le_bytes());
        data.extend_from_slice(&encoded_intent);
        data
    }

    #[test]
    fn builds_each_fixed_full_width_account_order() {
        let market = market();
        let owner = owner();
        assert!(!endow_v3_transaction(
            key(1),
            key(2),
            key(3),
            key(4),
            key(5),
            &market,
            owner,
            request(
                0,
                Intent::Endow {
                    market: Hash32::from_bytes(key(40)),
                    owner: Hash32::from_bytes(owner.owner),
                    amount: 1,
                },
            ),
        )
        .is_empty());
        assert!(!withdraw_cash_v3_transaction(
            key(1),
            key(2),
            key(3),
            &market,
            owner,
            key(34),
            request(
                0,
                Intent::WithdrawCash {
                    market: Hash32::from_bytes(key(40)),
                    owner: Hash32::from_bytes(owner.owner),
                    destination: Hash32::from_bytes(key(34)),
                    amount: 1,
                },
            ),
        )
        .is_empty());
        assert!(!complete_set_v3_transaction(
            key(1),
            key(2),
            key(3),
            &market,
            owner,
            request(
                0,
                Intent::Split {
                    market: Hash32::from_bytes(key(40)),
                    owner: Hash32::from_bytes(owner.owner),
                    quantity: 1,
                },
            ),
        )
        .is_empty());
    }

    #[test]
    fn includes_nonselected_outcome_mints_as_readonly_accounts() {
        let market = market();
        let owner = owner();
        assert!(!claim_representation_v3_transaction(
            key(1),
            key(2),
            key(3),
            &market,
            owner,
            key(34),
            0,
            request(
                0,
                Intent::Materialize {
                    market: Hash32::from_bytes(key(40)),
                    owner: Hash32::from_bytes(owner.owner),
                    destination: Hash32::from_bytes(key(34)),
                    outcome: 0,
                    quantity: 1,
                },
            ),
        )
        .is_empty());
        assert!(!redeem_external_v3_transaction(
            key(1),
            key(2),
            key(3),
            &market,
            owner.owner,
            key(35),
            key(36),
            key(37),
            1,
            request(
                0,
                Intent::RedeemExternal {
                    market: Hash32::from_bytes(key(40)),
                    claimant: Hash32::from_bytes(owner.owner),
                    source: Hash32::from_bytes(key(35)),
                    destination: Hash32::from_bytes(key(36)),
                    outcome: 1,
                    quantity: 1,
                },
            ),
        )
        .is_empty());
    }

    #[test]
    #[should_panic(expected = "distinct outcome mints")]
    fn refuses_duplicate_outcome_mint_roles() {
        let mut market = market();
        market.outcome_mints[1] = market.outcome_mints[0];
        let _ = claim_representation_v3_transaction(
            key(1),
            key(2),
            key(3),
            &market,
            owner(),
            key(34),
            0,
            request(
                0,
                Intent::Materialize {
                    market: Hash32::from_bytes(key(40)),
                    owner: Hash32::from_bytes(owner().owner),
                    destination: Hash32::from_bytes(key(34)),
                    outcome: 0,
                    quantity: 1,
                },
            ),
        );
    }

    #[test]
    fn admits_fee_payer_alias_for_transaction_signing_owner() {
        let market = market();
        let owner = owner();
        assert!(!complete_set_v3_transaction(
            owner.owner,
            key(2),
            key(3),
            &market,
            owner,
            request(
                0,
                Intent::Merge {
                    market: Hash32::from_bytes(key(40)),
                    owner: Hash32::from_bytes(owner.owner),
                    quantity: 1,
                },
            ),
        )
        .is_empty());
    }
}
