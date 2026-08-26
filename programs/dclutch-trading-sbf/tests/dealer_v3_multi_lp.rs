//! Adversarial integration coverage for canonical Dealer V3 multi-LP capital.

use dclutch_claims_svm::affine_batch_v2::AFFINE_BATCH_PLAN_MAGIC_V2;
use dclutch_custody_contract::{CompartmentV1, CustodyVaultSeedsV1};
use dclutch_dealer_codec::scenario::ClaimsInventoryObservation;
use dclutch_trading_sbf::dealer::{
    v3_multi_lp::{
        DEALER_LP_POSITION_BYTES_V3, DEALER_LP_POSITION_PDA_DOMAIN_V3,
        DealerLpAccountObservationV3, DealerLpPositionV3, MultiLpActionV3,
        MultiLpCollateralFrameV3, MultiLpContextV3, MultiLpIntentV3, prepare_multi_lp_v3,
    },
    v3_obligation::{
        DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
        DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
        DealerObligationProjectionV3,
    },
};
use solana_program::{hash::hash, pubkey::Pubkey};

fn obligation_bytes(values: &[u64], lp: u64) -> Vec<u8> {
    let mut bytes = vec![0; DEALER_OBLIGATION_HEADER_BYTES_V3 + values.len() * 8];
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

struct Fixture {
    trading: [u8; 32],
    context: MultiLpContextV3,
    principal_vault: [u8; 32],
    lp_address: [u8; 32],
    lp: [u8; DEALER_LP_POSITION_BYTES_V3],
    obligations: Vec<u8>,
}

fn fixture() -> Fixture {
    let trading = [9; 32];
    let custody = [13; 32];
    let obligation_account = Pubkey::find_program_address(
        &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &[5; 32]],
        &Pubkey::new_from_array(trading),
    )
    .0
    .to_bytes();
    let context = MultiLpContextV3 {
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
    };
    let principal_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new([1; 32], [6; 32], [5; 32], CompartmentV1::TradingPrincipal)
            .as_slices(),
        &Pubkey::new_from_array(custody),
    )
    .0
    .to_bytes();
    let lp_address = Pubkey::find_program_address(
        &[DEALER_LP_POSITION_PDA_DOMAIN_V3, &[5; 32], &[8; 32]],
        &Pubkey::new_from_array(trading),
    )
    .0
    .to_bytes();
    let mut lp = [0; DEALER_LP_POSITION_BYTES_V3];
    DealerLpPositionV3 {
        revision: 4,
        release_set: [6; 32],
        market: [1; 32],
        child_root: [5; 32],
        lp_owner: [8; 32],
        rent_refund: [8; 32],
        obligation_account,
        principal_shares: 20,
        generation: 2,
    }
    .encode_into(&mut lp)
    .expect("LP state");
    Fixture {
        trading,
        context,
        principal_vault,
        lp_address,
        lp,
        obligations: obligation_bytes(&[20, 20, 20], 20),
    }
}

#[test]
fn two_distinct_lp_accounts_share_one_canonical_obligation_owner() {
    let one = Pubkey::find_program_address(
        &[DEALER_LP_POSITION_PDA_DOMAIN_V3, &[5; 32], &[8; 32]],
        &Pubkey::new_from_array([9; 32]),
    )
    .0;
    let two = Pubkey::find_program_address(
        &[DEALER_LP_POSITION_PDA_DOMAIN_V3, &[5; 32], &[18; 32]],
        &Pubkey::new_from_array([9; 32]),
    )
    .0;
    let obligation = Pubkey::find_program_address(
        &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &[5; 32]],
        &Pubkey::new_from_array([9; 32]),
    )
    .0;
    assert_ne!(one, two);
    assert_ne!(one, obligation);
    assert_ne!(two, obligation);
}

#[test]
fn add_and_remove_are_exact_inverse_physical_profiles() {
    for (action, amount, expected_external, expected_principal, expected_lp) in [
        (MultiLpActionV3::Add, 7, 93, 37, 27),
        (MultiLpActionV3::Remove, 7, 107, 23, 13),
    ] {
        let f = fixture();
        let obligation = DealerObligationProjectionV3::decode(&f.obligations).expect("obligation");
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
        let mut equity_before = [0; 3];
        let mut equity_after = [0; 3];
        let mut post_obligation = vec![0; f.obligations.len()];
        let mut post_lp = [0; DEALER_LP_POSITION_BYTES_V3];
        let plan = prepare_multi_lp_v3(
            f.context,
            MultiLpCollateralFrameV3 {
                lp_external_account: [14; 32],
                lp_owner: [8; 32],
                lp_external_balance: 100,
                principal_vault: f.principal_vault,
                principal_balance: 30,
            },
            DealerLpAccountObservationV3 {
                address: f.lp_address,
                owner: f.trading,
                data: &f.lp,
            },
            obligation,
            claims,
            MultiLpIntentV3 {
                action,
                amount,
                expected_lp_revision: 4,
                expected_lp_digest: hash(&f.lp).to_bytes(),
            },
            &mut before,
            &mut after,
            &mut equity_before,
            &mut equity_after,
            &mut post_obligation,
            &mut post_lp,
        )
        .expect("scenario-solvent physical plan");
        assert_eq!(plan.custody.external_after, expected_external);
        assert_eq!(plan.custody.principal_after, expected_principal);
        assert_eq!(
            DealerLpPositionV3::decode(&post_lp)
                .expect("post LP")
                .principal_shares,
            expected_lp
        );
        assert_eq!(equity_before, equity_after);
        assert_ne!(
            plan.custody.request.to_bytes().expect("request")[..8],
            AFFINE_BATCH_PLAN_MAGIC_V2
        );
    }
}

#[test]
fn substituted_lp_owner_or_oversized_exit_refuses_before_state_candidates() {
    let f = fixture();
    let obligation = DealerObligationProjectionV3::decode(&f.obligations).expect("obligation");
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
    let mut equity_before = [0; 3];
    let mut equity_after = [0; 3];
    let mut post_obligation = vec![0xa5; f.obligations.len()];
    let mut post_lp = [0xa5; DEALER_LP_POSITION_BYTES_V3];
    let refusal = prepare_multi_lp_v3(
        f.context,
        MultiLpCollateralFrameV3 {
            lp_external_account: [14; 32],
            lp_owner: [18; 32],
            lp_external_balance: 100,
            principal_vault: f.principal_vault,
            principal_balance: 30,
        },
        DealerLpAccountObservationV3 {
            address: f.lp_address,
            owner: f.trading,
            data: &f.lp,
        },
        obligation,
        claims,
        MultiLpIntentV3 {
            action: MultiLpActionV3::Remove,
            amount: 21,
            expected_lp_revision: 4,
            expected_lp_digest: hash(&f.lp).to_bytes(),
        },
        &mut before,
        &mut after,
        &mut equity_before,
        &mut equity_after,
        &mut post_obligation,
        &mut post_lp,
    );
    assert!(refusal.is_err());
    assert!(post_obligation.iter().all(|byte| *byte == 0xa5));
    assert_eq!(post_lp, [0xa5; DEALER_LP_POSITION_BYTES_V3]);
}
