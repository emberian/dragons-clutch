// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_retirement::{
    plan_epoch_budget_retirement, AuthenticatedEpochBudgetDispositionV1, DeletableRentOwnerV1,
    EpochBudgetRetirementRequestV1, Identity32V1, RecipientBalanceBookV1, RecipientBalanceV1,
    RetirementErrorV2,
};

fn id(byte: u8) -> Identity32V1 {
    Identity32V1::new([byte; 32]).unwrap()
}

fn rent(payer: u8, principal: u64, donation_floor: u64) -> DeletableRentOwnerV1 {
    DeletableRentOwnerV1::from_persisted(id(payer), principal, donation_floor).unwrap()
}

fn disposition() -> AuthenticatedEpochBudgetDispositionV1 {
    AuthenticatedEpochBudgetDispositionV1::after_semantic_owner_validation(
        id(40),
        id(1),
        id(2),
        7,
        id(250),
        id(6),
        rent(3, 100, 5),
        13,
    )
    .unwrap()
}

fn recipients(reward_recipient: Identity32V1) -> RecipientBalanceBookV1 {
    RecipientBalanceBookV1 {
        entries: [
            Some(RecipientBalanceV1 {
                recipient: id(3),
                balance_before: 900,
            }),
            Some(RecipientBalanceV1 {
                recipient: reward_recipient,
                balance_before: 10,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(250),
                balance_before: 20,
            }),
            None,
        ],
    }
}

#[test]
fn terminal_budget_split_is_exact_and_atomic() {
    let reward_recipient = id(4);
    let before = disposition();
    let plan = plan_epoch_budget_retirement(EpochBudgetRetirementRequestV1 {
        disposition: before,
        reward_recipient,
        budget_balance: 125,
        recipient_balances: recipients(reward_recipient),
    })
    .unwrap();

    assert_eq!(before, disposition());
    assert_eq!(plan.budget_account, id(40));
    assert_eq!(plan.root_close_reward_lamports, 13);
    assert_eq!(plan.rent_principal_refund_lamports, 100);
    assert_eq!(plan.neutral_lamports, 12);
    assert_eq!(plan.budget_balance_after, 0);
    assert_eq!(
        plan.recipient_credits.get(id(3)).unwrap().credit_lamports,
        100
    );
    assert_eq!(
        plan.recipient_credits.get(id(3)).unwrap().balance_after,
        1_000
    );
    assert_eq!(
        plan.recipient_credits.get(id(4)).unwrap().credit_lamports,
        13
    );
    assert_eq!(plan.recipient_credits.get(id(4)).unwrap().balance_after, 23);
    assert_eq!(
        plan.recipient_credits.get(id(250)).unwrap().credit_lamports,
        12
    );
    assert_eq!(
        plan.recipient_credits.get(id(250)).unwrap().balance_after,
        32
    );
}

#[test]
fn closer_and_rent_payer_coalesce_before_checked_addition() {
    let coalesced = RecipientBalanceBookV1 {
        entries: [
            Some(RecipientBalanceV1 {
                recipient: id(3),
                balance_before: 900,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(250),
                balance_before: 20,
            }),
            None,
            None,
        ],
    };
    let plan = plan_epoch_budget_retirement(EpochBudgetRetirementRequestV1 {
        disposition: disposition(),
        reward_recipient: id(3),
        budget_balance: 125,
        recipient_balances: coalesced,
    })
    .unwrap();
    assert_eq!(
        plan.recipient_credits.get(id(3)).unwrap().credit_lamports,
        113
    );
    assert_eq!(
        plan.recipient_credits.get(id(3)).unwrap().balance_after,
        1_013
    );
}

#[test]
fn semantic_owner_handoff_rechecks_identity_and_funding_geometry() {
    let make = |account, generation, sink, funding_payer, rent, reward| {
        AuthenticatedEpochBudgetDispositionV1::after_semantic_owner_validation(
            account,
            id(1),
            id(2),
            generation,
            sink,
            funding_payer,
            rent,
            reward,
        )
    };
    assert_eq!(
        make(id(40), 0, id(250), id(3), rent(3, 100, 5), 13),
        Err(RetirementErrorV2::WrongGeneration)
    );
    assert_eq!(
        make(id(40), 7, id(250), id(3), rent(3, 100, 5), 0),
        Err(RetirementErrorV2::NonCanonicalState)
    );
    assert_eq!(
        make(id(40), 7, id(250), id(4), rent(3, 100, 5), 13),
        AuthenticatedEpochBudgetDispositionV1::after_semantic_owner_validation(
            id(40),
            id(1),
            id(2),
            7,
            id(250),
            id(4),
            rent(3, 100, 5),
            13,
        )
    );
    assert_eq!(
        make(id(40), 7, id(3), id(4), rent(3, 100, 5), 13),
        Err(RetirementErrorV2::PayerIsNeutralSink)
    );
    assert_eq!(
        make(id(40), 7, id(250), id(250), rent(3, 100, 5), 13),
        Err(RetirementErrorV2::PayerIsNeutralSink)
    );
    assert_eq!(
        make(id(3), 7, id(250), id(3), rent(3, 100, 5), 13),
        Err(RetirementErrorV2::AccountAlias)
    );
    assert_eq!(
        make(id(40), 7, id(250), id(3), rent(3, u64::MAX, 0), 1,),
        Err(RetirementErrorV2::ArithmeticOverflow)
    );
}

#[test]
fn shortfall_alias_missing_recipient_and_credit_overflow_refuse() {
    let reward_recipient = id(4);
    let request = |balance, reward, book| EpochBudgetRetirementRequestV1 {
        disposition: disposition(),
        reward_recipient: reward,
        budget_balance: balance,
        recipient_balances: book,
    };
    assert_eq!(
        plan_epoch_budget_retirement(request(117, reward_recipient, recipients(reward_recipient))),
        Err(RetirementErrorV2::AccountBalanceShortfall)
    );
    assert_eq!(
        plan_epoch_budget_retirement(request(125, id(250), recipients(reward_recipient))),
        Err(RetirementErrorV2::AccountAlias)
    );

    let mut source_alias = recipients(reward_recipient);
    source_alias.entries[3] = Some(RecipientBalanceV1 {
        recipient: id(40),
        balance_before: 0,
    });
    assert_eq!(
        plan_epoch_budget_retirement(request(125, reward_recipient, source_alias)),
        Err(RetirementErrorV2::AccountAlias)
    );

    let missing_reward = RecipientBalanceBookV1 {
        entries: [
            Some(RecipientBalanceV1 {
                recipient: id(3),
                balance_before: 900,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(250),
                balance_before: 20,
            }),
            None,
            None,
        ],
    };
    assert_eq!(
        plan_epoch_budget_retirement(request(125, reward_recipient, missing_reward)),
        Err(RetirementErrorV2::MissingRecipient)
    );

    let overflowing = RecipientBalanceBookV1 {
        entries: [
            Some(RecipientBalanceV1 {
                recipient: id(3),
                balance_before: u64::MAX,
            }),
            Some(RecipientBalanceV1 {
                recipient: reward_recipient,
                balance_before: 10,
            }),
            Some(RecipientBalanceV1 {
                recipient: id(250),
                balance_before: 20,
            }),
            None,
        ],
    };
    assert_eq!(
        plan_epoch_budget_retirement(request(125, reward_recipient, overflowing)),
        Err(RetirementErrorV2::ArithmeticOverflow)
    );
}

#[test]
fn every_lamport_is_conserved_between_distinct_compartments() {
    let plan = plan_epoch_budget_retirement(EpochBudgetRetirementRequestV1 {
        disposition: disposition(),
        reward_recipient: id(4),
        budget_balance: u64::MAX,
        recipient_balances: RecipientBalanceBookV1 {
            entries: [
                Some(RecipientBalanceV1 {
                    recipient: id(3),
                    balance_before: 0,
                }),
                Some(RecipientBalanceV1 {
                    recipient: id(4),
                    balance_before: 0,
                }),
                Some(RecipientBalanceV1 {
                    recipient: id(250),
                    balance_before: 0,
                }),
                None,
            ],
        },
    })
    .unwrap();
    assert_eq!(
        plan.rent_principal_refund_lamports
            .checked_add(plan.root_close_reward_lamports)
            .and_then(|value| value.checked_add(plan.neutral_lamports)),
        Some(u64::MAX)
    );
}
