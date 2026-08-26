//! Adversarial integration coverage for canonical Dealer V3 multi-LP capital.

use dclutch_capability_program_contract::set_v1::CapabilityProgramSetV1;
use dclutch_claims_svm::affine_batch_v2::AFFINE_BATCH_PLAN_MAGIC_V2;
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CompartmentV1, CustodyRequestV1, CustodyVaultSeedsV1,
    DELEGATED_CUSTODY_REQUEST_MAGIC_V2,
};
use dclutch_dealer_codec::scenario::ClaimsInventoryObservation;
use dclutch_effect_kernel::v3::ProgramV3 as EffectProgramV3;
use dclutch_trading_sbf::dealer::{
    v3_equity_operator::{
        DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3, DEALER_EQUITY_HEADER_BYTES_V3,
        DEALER_EQUITY_SELECTOR_OFFSET_V3, DealerEquityRequestV3, EquityOperatorErrorV3,
        EquityPoolChainProjectionV3, EquityRequestActionV3, EquityRequestIntentV3,
        build_equity_request_v3, materialize_equity_intent_v3, prepare_equity_request_v3,
    },
    v3_hot_artifact::{
        dealer_equity_effect_program_bytes_v3, dealer_equity_identity_count_v3,
        dealer_equity_scalar_count_v3, encode_dealer_equity_effect_program_v3,
        project_dealer_equity_hot_registers_v3,
    },
    v3_multi_lp::{
        DEALER_LP_POSITION_BYTES_V3, DEALER_LP_POSITION_PDA_DOMAIN_V3,
        DealerLpAccountObservationV3, DealerLpPositionV3, MultiLpActionV3,
        MultiLpCollateralFrameV3, MultiLpContextV3, MultiLpCustodyRequestV3, MultiLpIntentV3,
        prepare_multi_lp_v3,
    },
    v3_obligation::{
        DEALER_OBLIGATION_HEADER_BYTES_V3, DEALER_OBLIGATION_MAGIC_V3,
        DEALER_OBLIGATION_PDA_DOMAIN_V3, DEALER_OBLIGATION_VERSION_V3,
        DealerObligationProjectionV3,
    },
    v3_route::authenticate_dealer_equity_routes_v3,
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
    hoard_vault: [u8; 32],
    custody_authority: [u8; 32],
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
    let hoard_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new([1; 32], [6; 32], [1; 32], CompartmentV1::HoardPrincipal)
            .as_slices(),
        &Pubkey::new_from_array(custody),
    )
    .0
    .to_bytes();
    let custody_authority = Pubkey::find_program_address(
        &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &[1; 32], &[6; 32]],
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
        equity_shares: 20,
        generation: 2,
    }
    .encode_into(&mut lp)
    .expect("LP state");
    Fixture {
        trading,
        context,
        principal_vault,
        hoard_vault,
        custody_authority,
        lp_address,
        lp,
        obligations: obligation_bytes(&[0, 0, 0], 20),
    }
}

fn program_set(selector: u16) -> Vec<u8> {
    let mut bytes = vec![0; 72];
    bytes[..8].copy_from_slice(b"DCLTCPS1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&DEALER_EQUITY_SELECTOR_OFFSET_V3.to_le_bytes());
    bytes[16] = 2;
    bytes[18..20].copy_from_slice(&1_u16.to_le_bytes());
    bytes[32..36].copy_from_slice(&u32::from(selector).to_le_bytes());
    bytes[36..68].copy_from_slice(&[42; 32]);
    bytes
}

fn encode_custody_request(request: MultiLpCustodyRequestV3) -> Vec<u8> {
    let mut output = vec![0; request.encoded_len()];
    request.encode_into(&mut output).expect("Custody request");
    output
}

fn inactive_merge_template(cash: MultiLpCustodyRequestV3, f: &Fixture) -> CustodyRequestV1 {
    let mut merge = cash.custody();
    merge.source_compartment = CompartmentV1::HoardPrincipal;
    merge.destination_compartment = CompartmentV1::TradingPrincipal;
    merge.semantic.source_owner = [0; 32];
    merge.semantic.destination_owner = [0; 32];
    merge.source = f.hoard_vault;
    merge.destination = f.principal_vault;
    merge.source_vault_context = f.context.market;
    merge.destination_vault_context = f.context.child_root;
    merge.semantic.transfer_index = 1;
    merge.expected_revision = merge.expected_revision.checked_add(1).expect("revision");
    merge.resulting_revision = merge.expected_revision.checked_add(1).expect("revision");
    merge.amount = 1;
    merge
}

fn equity_effect(
    cash: MultiLpCustodyRequestV3,
    merge: CustodyRequestV1,
    signed_position_count: u32,
) -> Vec<u8> {
    let templates = [cash, MultiLpCustodyRequestV3::Canonical(merge)];
    let width = dealer_equity_effect_program_bytes_v3(MultiLpActionV3::Add, signed_position_count)
        .expect("Dealer effect width");
    let mut scratch = vec![0; width];
    let mut output = vec![0; width];
    encode_dealer_equity_effect_program_v3(
        MultiLpActionV3::Add,
        signed_position_count,
        &templates,
        &mut scratch,
        &mut output,
    )
    .expect("Dealer effect");
    output
}

#[test]
fn runtime_width_equity_request_is_chain_derived_and_rejoins_physical_intent() {
    let f = fixture();
    let obligation = DealerObligationProjectionV3::decode(&f.obligations).expect("obligation");
    let dealer_inventory = [0, 10, 20];
    let lp_inventory = [10, 10, 10];
    let dealer_claims = ClaimsInventoryObservation {
        market_id: [1; 32],
        product_id: [2; 32],
        liability_basis_id: [3; 32],
        position_owner: [4; 32],
        revision: 5,
        inventory: &dealer_inventory,
    };
    let lp_claims = ClaimsInventoryObservation {
        market_id: [1; 32],
        product_id: [2; 32],
        liability_basis_id: [3; 32],
        position_owner: [8; 32],
        revision: 6,
        inventory: &lp_inventory,
    };
    let collateral = MultiLpCollateralFrameV3 {
        lp_external_account: [14; 32],
        lp_owner: [8; 32],
        lp_external_balance: 100,
        lp_external_delegate: f.custody_authority,
        lp_external_delegated_amount: 10,
        principal_vault: f.principal_vault,
        principal_balance: 20,
        hoard_vault: f.hoard_vault,
        hoard_balance: 100,
    };
    let chain = EquityPoolChainProjectionV3 {
        trading_program: f.trading,
        release_set: f.context.release_set,
        market: f.context.market,
        child_root: f.context.child_root,
        obligation_address: f.context.obligation_account,
        obligation,
        lp_position_address: f.lp_address,
        lp_position: DealerLpPositionV3::decode(&f.lp).expect("LP"),
        lp_position_bytes: &f.lp,
        dealer_claims,
        lp_claims,
        product_record_digest: [15; 32],
        linked_basis_record_digest: [16; 32],
        claims_market_revision: 7,
        collateral,
        locked_capital_floor: f.context.locked_capital_floor,
        generation: f.context.generation,
        now: 20,
        expires_at: 25,
        terminal: false,
    };
    let set_bytes = program_set(DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3);
    let set = CapabilityProgramSetV1::decode(&set_bytes).expect("program set");
    let mut output = vec![0; 1024];
    let mut obligation_scratch = [0; 3];
    let mut builder_before = [0; 3];
    let mut builder_after = [0; 3];
    let mut builder_transferred = [0; 3];
    let mut builder_post_dealer = [0; 3];
    let mut builder_post_lp = [0; 3];
    let unsigned = build_equity_request_v3(
        chain,
        EquityRequestIntentV3::Contribute {
            collateral: 10,
            claims: &[0, 5, 10],
            minted_shares: 10,
        },
        set,
        &mut output,
        &mut obligation_scratch,
        &mut builder_before,
        &mut builder_after,
        &mut builder_transferred,
        &mut builder_post_dealer,
        &mut builder_post_lp,
    )
    .expect("proportional request");
    assert_eq!(unsigned.request_bytes, 944);
    assert_eq!(unsigned.selected_program.to_bytes(), [42; 32]);
    let request_bytes = output.get(..unsigned.request_bytes).expect("request bytes");
    let request = DealerEquityRequestV3::decode(request_bytes).expect("request");
    assert_eq!(request.action(), EquityRequestActionV3::Contribute);
    assert_eq!(request.selector(), DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3);
    assert_eq!(request.claims_packet().len(), 464);
    assert_eq!(
        request.claims_packet().as_ptr(),
        request_bytes[DEALER_EQUITY_HEADER_BYTES_V3..].as_ptr()
    );
    let mut decoded_claims = [0; 3];
    let intent = materialize_equity_intent_v3(request, chain, &mut decoded_claims).expect("intent");
    assert_eq!(
        intent,
        MultiLpIntentV3::Contribute {
            collateral: 10,
            claims: &[0, 5, 10],
            minted_shares: 10,
            expected_lp_revision: 4,
            expected_lp_digest: hash(&f.lp).to_bytes(),
        }
    );

    let mut physical_context = f.context;
    physical_context.parent_request_digest = hash(request_bytes).to_bytes();
    let mut request_claims = [0; 3];
    let mut physical_obligations = [0; 3];
    let mut before = [0; 3];
    let mut after = [0; 3];
    let mut transferred = [0; 3];
    let mut post_dealer_claims = [0; 3];
    let mut post_lp_claims = [0; 3];
    let mut post_obligation = vec![0; f.obligations.len()];
    let mut post_lp = [0; DEALER_LP_POSITION_BYTES_V3];
    let physical = prepare_equity_request_v3(
        request,
        chain,
        physical_context,
        &mut request_claims,
        &mut physical_obligations,
        &mut before,
        &mut after,
        &mut transferred,
        &mut post_dealer_claims,
        &mut post_lp_claims,
        &mut post_obligation,
        &mut post_lp,
    )
    .expect("request-to-physical join");
    assert_eq!(physical.share_delta, 10);
    assert_eq!(physical.principal_after, 30);
    assert_eq!(post_dealer_claims, [0, 15, 30]);
    assert_eq!(post_lp_claims, [10, 5, 0]);

    let cash = physical.custody[0].expect("cash Custody").request;
    let merge = inactive_merge_template(cash, &f);
    let cash_bytes = encode_custody_request(cash);
    let merge_bytes = merge.to_bytes().expect("merge request");
    let mut request_bank = Vec::with_capacity(cash_bytes.len() + merge_bytes.len());
    request_bank.extend_from_slice(&cash_bytes);
    request_bank.extend_from_slice(&merge_bytes);
    let mut scalars = vec![0; dealer_equity_scalar_count_v3(physical.action).expect("scalars")];
    let mut identities =
        vec![[0; 32]; dealer_equity_identity_count_v3(physical.action).expect("identities")];
    project_dealer_equity_hot_registers_v3(request, physical, &mut scalars, &mut identities)
        .expect("chain-derived Hot registers");
    let effect_bytes = equity_effect(cash, merge, 2);
    let effect = EffectProgramV3::decode(&effect_bytes).expect("Dealer Hot effect");
    let composition = authenticate_dealer_equity_routes_v3(
        effect,
        3,
        &scalars,
        &identities,
        &request_bank,
        request_bytes,
        request,
        physical,
    )
    .expect("cash then Claims route order");
    assert_eq!(composition.claims_route(), Some(1));
    assert_eq!(composition.custody().count(), 1);

    let mut reversed = effect_bytes.clone();
    reversed[32] = 1;
    assert!(
        authenticate_dealer_equity_routes_v3(
            EffectProgramV3::decode(&reversed).expect("reversed structural effect"),
            3,
            &scalars,
            &identities,
            &request_bank,
            request_bytes,
            request,
            physical,
        )
        .is_err()
    );

    let mut stale = chain;
    stale.collateral.principal_balance = 21;
    assert_eq!(
        dclutch_trading_sbf::dealer::v3_equity_operator::authenticate_equity_request_v3(
            request, stale,
        ),
        Err(EquityOperatorErrorV3::InvalidProjection)
    );
}

#[test]
fn unsigned_equity_builder_refuses_dilution_before_emitting_request() {
    let f = fixture();
    let obligation = DealerObligationProjectionV3::decode(&f.obligations).expect("obligation");
    let dealer_inventory = [0, 10, 20];
    let lp_inventory = [10, 10, 10];
    let chain = EquityPoolChainProjectionV3 {
        trading_program: f.trading,
        release_set: f.context.release_set,
        market: f.context.market,
        child_root: f.context.child_root,
        obligation_address: f.context.obligation_account,
        obligation,
        lp_position_address: f.lp_address,
        lp_position: DealerLpPositionV3::decode(&f.lp).expect("LP"),
        lp_position_bytes: &f.lp,
        dealer_claims: ClaimsInventoryObservation {
            market_id: [1; 32],
            product_id: [2; 32],
            liability_basis_id: [3; 32],
            position_owner: [4; 32],
            revision: 5,
            inventory: &dealer_inventory,
        },
        lp_claims: ClaimsInventoryObservation {
            market_id: [1; 32],
            product_id: [2; 32],
            liability_basis_id: [3; 32],
            position_owner: [8; 32],
            revision: 6,
            inventory: &lp_inventory,
        },
        product_record_digest: [15; 32],
        linked_basis_record_digest: [16; 32],
        claims_market_revision: 7,
        collateral: MultiLpCollateralFrameV3 {
            lp_external_account: [14; 32],
            lp_owner: [8; 32],
            lp_external_balance: 100,
            lp_external_delegate: f.custody_authority,
            lp_external_delegated_amount: 10,
            principal_vault: f.principal_vault,
            principal_balance: 20,
            hoard_vault: f.hoard_vault,
            hoard_balance: 100,
        },
        locked_capital_floor: f.context.locked_capital_floor,
        generation: f.context.generation,
        now: 20,
        expires_at: 25,
        terminal: false,
    };
    let set_bytes = program_set(DEALER_EQUITY_CONTRIBUTE_P2_SELECTOR_V3);
    let set = CapabilityProgramSetV1::decode(&set_bytes).expect("program set");
    let mut output = vec![0xa5; 1024];
    let mut obligation_scratch = [0; 3];
    let mut before = [0; 3];
    let mut after = [0; 3];
    let mut transferred = [0; 3];
    let mut post_dealer = [0; 3];
    let mut post_lp = [0; 3];
    assert_eq!(
        build_equity_request_v3(
            chain,
            EquityRequestIntentV3::Contribute {
                collateral: 10,
                claims: &[0, 0, 0],
                minted_shares: 10,
            },
            set,
            &mut output,
            &mut obligation_scratch,
            &mut before,
            &mut after,
            &mut transferred,
            &mut post_dealer,
            &mut post_lp,
        ),
        Err(EquityOperatorErrorV3::InvalidIntent)
    );
    assert!(output.iter().all(|value| *value == 0xa5));
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
fn proportional_contribution_and_redemption_are_physical() {
    for (action, expected_external, expected_principal, expected_lp) in [
        (MultiLpActionV3::Add, 90, 30, 30),
        (MultiLpActionV3::Remove, 110, 10, 10),
    ] {
        let f = fixture();
        let obligation = DealerObligationProjectionV3::decode(&f.obligations).expect("obligation");
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
            inventory: &[10, 10, 10],
        };
        let intent = match action {
            MultiLpActionV3::Add => MultiLpIntentV3::Contribute {
                collateral: 10,
                claims: &[0, 5, 10],
                minted_shares: 10,
                expected_lp_revision: 4,
                expected_lp_digest: hash(&f.lp).to_bytes(),
            },
            MultiLpActionV3::Remove => MultiLpIntentV3::Redeem {
                burned_shares: 10,
                expected_lp_revision: 4,
                expected_lp_digest: hash(&f.lp).to_bytes(),
            },
        };
        let mut obligations = [0; 3];
        let mut before = [0; 3];
        let mut after = [0; 3];
        let mut transferred = [0; 3];
        let mut post_dealer_claims = [0; 3];
        let mut post_lp_claims = [0; 3];
        let mut post_obligation = vec![0; f.obligations.len()];
        let mut post_lp = [0; DEALER_LP_POSITION_BYTES_V3];
        let plan = prepare_multi_lp_v3(
            f.context,
            MultiLpCollateralFrameV3 {
                lp_external_account: [14; 32],
                lp_owner: [8; 32],
                lp_external_balance: 100,
                lp_external_delegate: f.custody_authority,
                lp_external_delegated_amount: 10,
                principal_vault: f.principal_vault,
                principal_balance: 20,
                hoard_vault: f.hoard_vault,
                hoard_balance: 100,
            },
            DealerLpAccountObservationV3 {
                address: f.lp_address,
                owner: f.trading,
                data: &f.lp,
            },
            obligation,
            claims,
            lp_claims,
            intent,
            &mut obligations,
            &mut before,
            &mut after,
            &mut transferred,
            &mut post_dealer_claims,
            &mut post_lp_claims,
            &mut post_obligation,
            &mut post_lp,
        )
        .expect("scenario-solvent physical plan");
        assert_eq!(plan.external_after, expected_external);
        assert_eq!(plan.principal_after, expected_principal);
        assert_eq!(
            DealerLpPositionV3::decode(&post_lp)
                .expect("post LP")
                .equity_shares,
            expected_lp
        );
        assert_ne!(before, after);
        let first = plan.custody[0].expect("cash Custody effect");
        match (action, first.request) {
            (MultiLpActionV3::Add, MultiLpCustodyRequestV3::Delegated(request)) => {
                let encoded = request.encode().expect("delegated request");
                assert_eq!(encoded[..8], DELEGATED_CUSTODY_REQUEST_MAGIC_V2);
                assert_eq!(request.delegate_before, f.custody_authority);
                assert_eq!((request.total_debit, request.allowance_before), (10, 10));
                assert!(request.terminal);
            }
            (MultiLpActionV3::Remove, MultiLpCustodyRequestV3::Canonical(request)) => {
                assert_ne!(
                    request.to_bytes().expect("request")[..8],
                    AFFINE_BATCH_PLAN_MAGIC_V2
                );
            }
            _ => panic!("action-specific Custody successor"),
        }
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
    let mut obligations = [0; 3];
    let mut before = [0; 3];
    let mut after = [0; 3];
    let mut transferred = [0; 3];
    let mut post_dealer_claims = [0; 3];
    let mut post_lp_claims = [0; 3];
    let mut post_obligation = vec![0xa5; f.obligations.len()];
    let mut post_lp = [0xa5; DEALER_LP_POSITION_BYTES_V3];
    let refusal = prepare_multi_lp_v3(
        f.context,
        MultiLpCollateralFrameV3 {
            lp_external_account: [14; 32],
            lp_owner: [18; 32],
            lp_external_balance: 100,
            lp_external_delegate: [0; 32],
            lp_external_delegated_amount: 0,
            principal_vault: f.principal_vault,
            principal_balance: 20,
            hoard_vault: f.hoard_vault,
            hoard_balance: 100,
        },
        DealerLpAccountObservationV3 {
            address: f.lp_address,
            owner: f.trading,
            data: &f.lp,
        },
        obligation,
        claims,
        lp_claims,
        MultiLpIntentV3::Redeem {
            burned_shares: 21,
            expected_lp_revision: 4,
            expected_lp_digest: hash(&f.lp).to_bytes(),
        },
        &mut obligations,
        &mut before,
        &mut after,
        &mut transferred,
        &mut post_dealer_claims,
        &mut post_lp_claims,
        &mut post_obligation,
        &mut post_lp,
    );
    assert!(refusal.is_err());
    assert!(post_obligation.iter().all(|byte| *byte == 0xa5));
    assert_eq!(post_lp, [0xa5; DEALER_LP_POSITION_BYTES_V3]);
}
