//! Canonical host transaction builders for the full-width collateral routes.
//!
//! These builders do not provision state and do not reinterpret any lowered
//! fixture address. Callers must supply exact Product, General, collateral,
//! and claim-plane accounts produced by the live founding flow.

use super::{budget_instruction, transaction, Instruction, Message, COMPUTE_UNIT_CEILING};
use clutch_sbf::instructions::{
    claim_representation_v3, collateral_cash_v3, complete_set_v3, external_redemption_v3,
};

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

fn build(
    fee_payer: [u8; 32],
    program: [u8; 32],
    compute_budget: [u8; 32],
    writable_signers: &[[u8; 32]],
    readonly_signers: &[[u8; 32]],
    writable: &[[u8; 32]],
    readonly_roles: &[[u8; 32]],
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
        ],
    );
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
        ],
    );
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
        &roles,
        data,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
            vec![1],
        )
        .is_empty());
        assert!(!withdraw_cash_v3_transaction(
            key(1),
            key(2),
            key(3),
            &market,
            owner,
            key(34),
            vec![2],
        )
        .is_empty());
        assert!(
            !complete_set_v3_transaction(key(1), key(2), key(3), &market, owner, vec![3],)
                .is_empty()
        );
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
            vec![4],
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
            vec![5],
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
            vec![4],
        );
    }
}
