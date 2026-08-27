use super::*;

fn id(byte: u8) -> Identity {
    [byte; 32]
}

fn policy() -> Policy {
    Policy {
        market_id: id(1),
        release_set_id: id(2),
        dealer_id: id(3),
        fee_recipient_id: id(5),
        unwind_recipient_id: id(6),
        outcome_count: 2,
        quote_scale: 100,
        fee_numerator: 1,
        fee_denominator: 100,
        minimum_work_funding: 50,
        replacement_delay: 5,
    }
}

fn candidate(candidate_id: Identity, revision: u64, valid_from: u64) -> [u8; CANDIDATE_BYTES] {
    let bids = [CurveBand {
        capacity: 100,
        price_numerator: 40,
    }];
    let asks = [CurveBand {
        capacity: 100,
        price_numerator: 60,
    }];
    let curves = [
        CurveInput {
            bids: &bids,
            asks: &asks,
        },
        CurveInput {
            bids: &bids,
            asks: &asks,
        },
    ];
    let minimum_inventory = [0, 0];
    let maximum_inventory = [100, 100];
    let mut output = [0_u8; CANDIDATE_BYTES];
    encode_candidate(
        &mut output,
        CandidateInput {
            candidate_id,
            revision,
            valid_from,
            expires_at: 2_000,
            quote_reserve_floor: 100,
            work_funding: 100,
            work_reward: 2,
            minimum_inventory: &minimum_inventory,
            maximum_inventory: &maximum_inventory,
            curves: &curves,
        },
    )
    .unwrap();
    output
}

#[test]
fn public_candidate_encoder_refuses_hostile_curves_before_mutation() {
    let bids = [CurveBand {
        capacity: 10,
        price_numerator: 70,
    }];
    let asks = [CurveBand {
        capacity: 10,
        price_numerator: 60,
    }];
    let curves = [CurveInput {
        bids: &bids,
        asks: &asks,
    }];
    let minimum = [0];
    let maximum = [10];
    let input = CandidateInput {
        candidate_id: id(1),
        revision: 1,
        valid_from: 1,
        expires_at: 2,
        quote_reserve_floor: 0,
        work_funding: 10,
        work_reward: 1,
        minimum_inventory: &minimum,
        maximum_inventory: &maximum,
        curves: &curves,
    };
    let mut output = [0xa5; CANDIDATE_BYTES];
    assert_eq!(
        encode_candidate(&mut output, input),
        Err(Error::InvalidCurve)
    );
    assert_eq!(output, [0xa5; CANDIDATE_BYTES]);
}

fn receipt() -> ReleaseReceipt {
    ReleaseReceipt {
        registry_program: id(7),
        release_set_id: id(2),
        program: id(8),
        artifact_release: id(9),
        semantic_release: id(10),
    }
}

fn initial_state() -> State {
    let mut inventory = [0_u64; MAX_OUTCOMES];
    inventory[..2].copy_from_slice(&[50, 50]);
    State {
        phase: Phase::Open,
        outcome_count: 2,
        winner: 0,
        active_candidate_id: id(11),
        pending_candidate_id: [0; 32],
        release_set_id: id(2),
        active_revision: 1,
        pending_revision: 0,
        state_revision: 1,
        inventory,
        buy_used: [0; MAX_OUTCOMES],
        sell_used: [0; MAX_OUTCOMES],
        buy_quote_paid: [0; MAX_OUTCOMES],
        sell_quote_paid: [0; MAX_OUTCOMES],
        fee_base: 0,
        fee_paid: 0,
        quote_custody: 1_000,
        fee_custody: 0,
        liveness_custody: 100,
        active_work_remaining: 100,
        pending_work_funding: 0,
    }
}

fn base_request(action: Action, state: State) -> Request {
    Request {
        action,
        side: Side::TakerBuys,
        outcome: 0,
        expected_state_revision: state.state_revision,
        // The canonical request for an action carries the slot only where the
        // action's transition reads one. `execute` round-trips every request
        // through `to_bytes`, so a helper that stamped 10 everywhere would make
        // the five padding actions unencodable.
        now: match action.now_discipline() {
            NowDisciplineV1::CanonicalZero => 0,
            NowDisciplineV1::ExecutionSlot => 10,
        },
        quantity: 0,
        expected_candidate_id: state.active_candidate_id,
        actor_id: [0; 32],
        replacement_candidate_id: [0; 32],
        expected_candidate_revision: state.active_revision,
    }
}

fn execute(
    state: State,
    request: Request,
    active: &[u8; CANDIDATE_BYTES],
    pending: Option<&[u8; CANDIDATE_BYTES]>,
    proposed: Option<&[u8; CANDIDATE_BYTES]>,
) -> Result<Transition> {
    let policy = policy().to_bytes().unwrap();
    let receipt = receipt().to_bytes().unwrap();
    let state = state.to_bytes().unwrap();
    let request = request.to_bytes().unwrap();
    interpret(Inputs {
        policy: &policy,
        active_candidate: active,
        pending_candidate: pending.map(<[_; CANDIDATE_BYTES]>::as_slice),
        proposed_candidate: proposed.map(<[_; CANDIDATE_BYTES]>::as_slice),
        release_receipt: &receipt,
        state: &state,
        request: &request,
    })
}

#[test]
fn fixed_widths_and_round_trips_are_exact() {
    assert_eq!(
        (POLICY_BYTES, CANDIDATE_BYTES, STATE_BYTES),
        (216, 4_576, 840)
    );
    assert_eq!((RECEIPT_BYTES, REQUEST_BYTES), (176, 144));
    assert_eq!(Policy::decode(&policy().to_bytes().unwrap()), Ok(policy()));
    assert_eq!(
        ReleaseReceipt::decode(&receipt().to_bytes().unwrap()),
        Ok(receipt())
    );
    assert_eq!(
        State::decode(&initial_state().to_bytes().unwrap()),
        Ok(initial_state())
    );
    let request = Request {
        quantity: 10,
        ..base_request(Action::Fill, initial_state())
    };
    assert_eq!(Request::decode(&request.to_bytes().unwrap()), Ok(request));
    assert_eq!(generated::REQUEST_EXAMPLE.len(), REQUEST_BYTES);
    let generated_request = Request::decode(&generated::REQUEST_EXAMPLE).unwrap();
    assert_eq!(generated_request.action, Action::Fill);
    assert_eq!(generated_request.expected_state_revision, 7);
    assert_eq!(
        (generated_request.now, generated_request.quantity),
        (11, 13)
    );
    assert_eq!(generated_request.expected_candidate_revision, 9);
}

#[test]
fn buy_fill_executes_exact_claim_custody_fee_and_work_plan() {
    let active = candidate(id(11), 1, 0);
    let state = initial_state();
    let request = Request {
        quantity: 10,
        ..base_request(Action::Fill, state)
    };
    let transition = execute(state, request, &active, None, None).unwrap();
    assert_eq!(transition.post.inventory[0], 40);
    assert_eq!(transition.post.buy_used[0], 10);
    assert_eq!(transition.post.buy_quote_paid[0], 6);
    assert_eq!(transition.post.quote_custody, 1_006);
    assert_eq!((transition.post.fee_base, transition.post.fee_paid), (6, 1));
    assert_eq!(transition.post.fee_custody, 1);
    assert_eq!(transition.post.active_work_remaining, 98);
    assert_eq!(transition.post.liveness_custody, 98);
    assert_eq!(
        transition.plan.claim,
        ClaimAction::Transfer {
            side: Side::TakerBuys,
            outcome: 0,
            quantity: 10
        }
    );
    assert_eq!(
        transition.plan.custody,
        [
            Some(CustodyTransfer {
                source: CustodyRole::TakerQuote,
                destination: CustodyRole::DealerQuote,
                amount: 6,
            }),
            Some(CustodyTransfer {
                source: CustodyRole::TakerQuote,
                destination: CustodyRole::FeeVault,
                amount: 1,
            }),
            Some(CustodyTransfer {
                source: CustodyRole::LivenessVault,
                destination: CustodyRole::Executor,
                amount: 2,
            }),
        ]
    );
}

#[test]
fn owner_can_add_and_remove_quote_or_claim_liquidity_without_touching_fees() {
    let active = candidate(id(11), 1, 0);
    let initial = initial_state();
    let add_quote = execute(
        initial,
        Request {
            action: Action::AddLiquidity,
            outcome: policy().outcome_count,
            now: 0,
            quantity: 250,
            actor_id: policy().dealer_id,
            ..base_request(Action::AddLiquidity, initial)
        },
        &active,
        None,
        None,
    )
    .expect("add quote principal");
    assert_eq!(add_quote.post.quote_custody, 1_250);
    assert_eq!(add_quote.post.fee_custody, 0);
    assert_eq!(
        add_quote.plan.custody[0],
        Some(CustodyTransfer {
            source: CustodyRole::DealerOwner,
            destination: CustodyRole::DealerQuote,
            amount: 250,
        })
    );

    let add_claim = execute(
        add_quote.post,
        Request {
            action: Action::AddLiquidity,
            outcome: 1,
            now: 0,
            quantity: 25,
            actor_id: policy().dealer_id,
            ..base_request(Action::AddLiquidity, add_quote.post)
        },
        &active,
        None,
        None,
    )
    .expect("add claim inventory");
    assert_eq!(add_claim.post.inventory[..2], [50, 75]);
    assert_eq!(
        add_claim.plan.claim,
        ClaimAction::AdjustLiquidity {
            add: true,
            outcome: 1,
            quantity: 25,
        }
    );

    let remove_claim = execute(
        add_claim.post,
        Request {
            action: Action::RemoveLiquidity,
            outcome: 1,
            now: 0,
            quantity: 10,
            actor_id: policy().dealer_id,
            ..base_request(Action::RemoveLiquidity, add_claim.post)
        },
        &active,
        None,
        None,
    )
    .expect("remove claim inventory");
    assert_eq!(remove_claim.post.inventory[..2], [50, 65]);

    let remove_quote = execute(
        remove_claim.post,
        Request {
            action: Action::RemoveLiquidity,
            outcome: policy().outcome_count,
            now: 0,
            quantity: 1_150,
            actor_id: policy().dealer_id,
            ..base_request(Action::RemoveLiquidity, remove_claim.post)
        },
        &active,
        None,
        None,
    )
    .expect("remove quote down to exact floor");
    assert_eq!(remove_quote.post.quote_custody, 100);
    assert_eq!(
        (remove_quote.post.fee_base, remove_quote.post.fee_paid),
        (0, 0)
    );
}

#[test]
fn liquidity_changes_refuse_foreign_owner_risk_escape_and_quote_floor() {
    let active = candidate(id(11), 1, 0);
    let initial = initial_state();
    for request in [
        Request {
            action: Action::AddLiquidity,
            outcome: 0,
            now: 0,
            quantity: 1,
            actor_id: id(99),
            ..base_request(Action::AddLiquidity, initial)
        },
        Request {
            action: Action::AddLiquidity,
            outcome: 0,
            now: 0,
            quantity: 51,
            actor_id: policy().dealer_id,
            ..base_request(Action::AddLiquidity, initial)
        },
        Request {
            action: Action::RemoveLiquidity,
            outcome: policy().outcome_count,
            now: 0,
            quantity: 901,
            actor_id: policy().dealer_id,
            ..base_request(Action::RemoveLiquidity, initial)
        },
    ] {
        assert!(execute(initial, request, &active, None, None).is_err());
    }
}

#[test]
fn fill_fragmentation_cannot_change_quote_or_fee_totals() {
    let active = candidate(id(11), 1, 0);
    let initial = initial_state();
    let first = execute(
        initial,
        Request {
            quantity: 4,
            ..base_request(Action::Fill, initial)
        },
        &active,
        None,
        None,
    )
    .unwrap()
    .post;
    let second = execute(
        first,
        Request {
            quantity: 6,
            ..base_request(Action::Fill, first)
        },
        &active,
        None,
        None,
    )
    .unwrap()
    .post;
    let whole = execute(
        initial,
        Request {
            quantity: 10,
            ..base_request(Action::Fill, initial)
        },
        &active,
        None,
        None,
    )
    .unwrap()
    .post;
    assert_eq!(second.inventory, whole.inventory);
    assert_eq!(second.buy_quote_paid, whole.buy_quote_paid);
    assert_eq!(
        (second.fee_base, second.fee_paid),
        (whole.fee_base, whole.fee_paid)
    );
    assert_eq!((second.quote_custody, second.fee_custody), (1_006, 1));
}

#[test]
fn sell_fill_pays_fee_from_gross_without_crossing_quote_floor() {
    let active = candidate(id(11), 1, 0);
    let initial = initial_state();
    let transition = execute(
        initial,
        Request {
            side: Side::TakerSells,
            quantity: 10,
            ..base_request(Action::Fill, initial)
        },
        &active,
        None,
        None,
    )
    .unwrap();
    assert_eq!(transition.post.inventory[0], 60);
    assert_eq!(transition.post.sell_quote_paid[0], 4);
    assert_eq!(transition.post.quote_custody, 996);
    assert_eq!(transition.post.fee_custody, 1);
    assert_eq!(
        transition.plan.custody,
        [
            Some(CustodyTransfer {
                source: CustodyRole::DealerQuote,
                destination: CustodyRole::TakerQuote,
                amount: 3,
            }),
            Some(CustodyTransfer {
                source: CustodyRole::DealerQuote,
                destination: CustodyRole::FeeVault,
                amount: 1,
            }),
            Some(CustodyTransfer {
                source: CustodyRole::LivenessVault,
                destination: CustodyRole::Executor,
                amount: 2,
            }),
        ]
    );
}

#[test]
fn replacement_is_delayed_revision_ordered_and_prepaid() {
    let active = candidate(id(11), 1, 0);
    let next = candidate(id(12), 2, 20);
    let initial = initial_state();
    let scheduled = execute(
        initial,
        Request {
            actor_id: policy().dealer_id,
            replacement_candidate_id: id(12),
            ..base_request(Action::ScheduleReplacement, initial)
        },
        &active,
        None,
        Some(&next),
    )
    .unwrap();
    assert_eq!(scheduled.post.pending_candidate_id, id(12));
    assert_eq!(scheduled.post.liveness_custody, 200);
    assert_eq!(
        scheduled.plan.custody[0],
        Some(CustodyTransfer {
            source: CustodyRole::DealerOwner,
            destination: CustodyRole::LivenessVault,
            amount: 100,
        })
    );
    let too_early = Request {
        now: 19,
        replacement_candidate_id: id(12),
        ..base_request(Action::ActivateReplacement, scheduled.post)
    };
    assert_eq!(
        execute(scheduled.post, too_early, &active, Some(&next), None),
        Err(Error::StaleCoordinate)
    );
    let activated = execute(
        scheduled.post,
        Request {
            now: 20,
            replacement_candidate_id: id(12),
            ..base_request(Action::ActivateReplacement, scheduled.post)
        },
        &active,
        Some(&next),
        None,
    )
    .unwrap();
    assert_eq!(activated.post.active_candidate_id, id(12));
    assert_eq!(activated.post.active_revision, 2);
    assert_eq!(activated.post.pending_candidate_id, [0; 32]);
    assert_eq!(activated.post.liveness_custody, 100);
    assert_eq!(activated.post.buy_used, [0; MAX_OUTCOMES]);
}

#[test]
fn terminal_unwind_redeems_winner_burns_loser_and_closes_custody() {
    let active = candidate(id(11), 1, 0);
    let initial = initial_state();
    let terminal = execute(
        initial,
        Request {
            actor_id: policy().market_id,
            outcome: 0,
            ..base_request(Action::EnterTerminal, initial)
        },
        &active,
        None,
        None,
    )
    .unwrap()
    .post;
    assert_eq!(
        execute(
            initial,
            Request {
                actor_id: id(99),
                outcome: 0,
                ..base_request(Action::EnterTerminal, initial)
            },
            &active,
            None,
            None,
        ),
        Err(Error::InvalidPhase)
    );
    let winner = execute(
        terminal,
        Request {
            outcome: 0,
            quantity: 50,
            ..base_request(Action::Unwind, terminal)
        },
        &active,
        None,
        None,
    )
    .unwrap();
    assert_eq!(winner.post.inventory[..2], [0, 50]);
    assert_eq!(winner.post.quote_custody, 1_050);
    assert_eq!(
        winner.plan.claim,
        ClaimAction::Redeem {
            outcome: 0,
            quantity: 50,
            payout: 50
        }
    );
    let loser = execute(
        winner.post,
        Request {
            outcome: 1,
            quantity: 50,
            ..base_request(Action::Unwind, winner.post)
        },
        &active,
        None,
        None,
    )
    .unwrap();
    assert_eq!(loser.post.inventory[..2], [0, 0]);
    assert_eq!(loser.post.quote_custody, 1_050);
    let retired = execute(
        loser.post,
        Request {
            now: 0,
            ..base_request(Action::Retire, loser.post)
        },
        &active,
        None,
        None,
    )
    .unwrap();
    assert_eq!(retired.post.phase, Phase::Retired);
    assert_eq!(
        (
            retired.post.quote_custody,
            retired.post.fee_custody,
            retired.post.liveness_custody
        ),
        (0, 0, 0)
    );
    assert_eq!(
        retired.plan.custody[0],
        Some(CustodyTransfer {
            source: CustodyRole::DealerQuote,
            destination: CustodyRole::UnwindRecipient,
            amount: 1_050,
        })
    );
}

#[test]
fn retirement_routes_all_three_custody_compartments_to_distinct_owners() {
    let active = candidate(id(11), 1, 0);
    let mut terminal = initial_state();
    terminal.phase = Phase::Terminal;
    terminal.inventory = [0; MAX_OUTCOMES];
    terminal.quote_custody = 1_050;
    terminal.fee_base = 6;
    terminal.fee_paid = 1;
    terminal.fee_custody = 1;
    terminal.active_work_remaining = 96;
    terminal.liveness_custody = 96;
    let retired = execute(
        terminal,
        Request {
            now: 0,
            ..base_request(Action::Retire, terminal)
        },
        &active,
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        retired.plan.custody,
        [
            Some(CustodyTransfer {
                source: CustodyRole::DealerQuote,
                destination: CustodyRole::UnwindRecipient,
                amount: 1_050,
            }),
            Some(CustodyTransfer {
                source: CustodyRole::FeeVault,
                destination: CustodyRole::FeeRecipient,
                amount: 1,
            }),
            Some(CustodyTransfer {
                source: CustodyRole::LivenessVault,
                destination: CustodyRole::DealerOwner,
                amount: 96,
            }),
        ]
    );
}

#[test]
fn hostile_release_padding_curve_counter_funding_and_overflow_refuse() {
    let active = candidate(id(11), 1, 0);
    let initial = initial_state();
    let request = Request {
        quantity: 10,
        ..base_request(Action::Fill, initial)
    };
    let policy_bytes = policy().to_bytes().unwrap();
    let state_bytes = initial.to_bytes().unwrap();
    let request_bytes = request.to_bytes().unwrap();

    let mut bad_receipt = receipt().to_bytes().unwrap();
    bad_receipt[generated::RECEIPT_FLAGS_OFFSET] = 1;
    assert_eq!(
        interpret(Inputs {
            policy: &policy_bytes,
            active_candidate: &active,
            pending_candidate: None,
            proposed_candidate: None,
            release_receipt: &bad_receipt,
            state: &state_bytes,
            request: &request_bytes,
        }),
        Err(Error::UnknownTag)
    );

    let mut bad_curve = active;
    bad_curve[curve_offset(0) + generated::CURVE_RESERVED_OFFSET] = 1;
    assert_eq!(
        execute(initial, request, &bad_curve, None, None),
        Err(Error::NonCanonicalPadding)
    );

    let mut reset_paid = initial;
    reset_paid.buy_used[0] = 4;
    assert_eq!(
        execute(reset_paid, request, &active, None, None),
        Err(Error::InvalidCurve)
    );

    let mut underfunded = initial;
    underfunded.active_work_remaining = 1;
    underfunded.liveness_custody = 1;
    assert_eq!(
        execute(underfunded, request, &active, None, None),
        Err(Error::Underfunded)
    );

    let mut overflow = initial;
    overflow.quote_custody = u64::MAX;
    assert_eq!(
        execute(overflow, request, &active, None, None),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn hostile_lengths_tags_reserved_bytes_and_stale_revision_refuse() {
    assert_eq!(Policy::decode(&[]), Err(Error::InvalidLength));
    let mut policy_bytes = policy().to_bytes().unwrap();
    policy_bytes[generated::POLICY_RESERVED_OFFSET] = 1;
    assert_eq!(
        Policy::decode(&policy_bytes),
        Err(Error::NonCanonicalPadding)
    );

    let mut request = Request {
        quantity: 10,
        ..base_request(Action::Fill, initial_state())
    }
    .to_bytes()
    .unwrap();
    request[generated::REQUEST_ACTION_OFFSET] = 99;
    assert_eq!(Request::decode(&request), Err(Error::UnknownTag));

    let active = candidate(id(11), 1, 0);
    let initial = initial_state();
    let stale = Request {
        expected_state_revision: 99,
        quantity: 10,
        ..base_request(Action::Fill, initial)
    };
    assert_eq!(
        execute(initial, stale, &active, None, None),
        Err(Error::StaleCoordinate)
    );
}

/// Every action, both directions, over the wire.
///
/// The `now` coordinate is padding for the five commands that carry no slot in
/// `DClutchSemantics.DealerLiquidity.Command` and a live slot for the three
/// that do. Before `Action::now_discipline` owned that fact, the shape rule and
/// the SBF adapter's `authenticate_clock` each restated a piece of it and
/// disagreed: `AddLiquidity` and `RemoveLiquidity` needed `now == 0` to encode
/// and `now == clock.slot` to authenticate, which no slot the chain can offer
/// satisfies. This walks all eight actions so no future action can be added to
/// one statement and forgotten in the other.
#[test]
fn every_action_states_its_now_discipline_once_and_over_the_wire() {
    const ALL: [Action; 8] = [
        Action::ScheduleReplacement,
        Action::ActivateReplacement,
        Action::Fill,
        Action::EnterTerminal,
        Action::Unwind,
        Action::Retire,
        Action::AddLiquidity,
        Action::RemoveLiquidity,
    ];
    let disciplines = ALL.map(Action::now_discipline);
    assert_eq!(
        disciplines,
        [
            NowDisciplineV1::ExecutionSlot, // ScheduleReplacement
            NowDisciplineV1::ExecutionSlot, // ActivateReplacement
            NowDisciplineV1::ExecutionSlot, // Fill
            NowDisciplineV1::CanonicalZero, // EnterTerminal
            NowDisciplineV1::CanonicalZero, // Unwind
            NowDisciplineV1::CanonicalZero, // Retire
            NowDisciplineV1::CanonicalZero, // AddLiquidity
            NowDisciplineV1::CanonicalZero, // RemoveLiquidity
        ]
    );

    for action in ALL {
        let canonical = shape_canonical(action);
        let bytes = canonical
            .to_bytes()
            .unwrap_or_else(|error| panic!("{action:?} is shape-canonical: {error:?}"));
        assert_eq!(Request::decode(&bytes), Ok(canonical), "{action:?}");

        // The hostile client from the AddLiquidity/RemoveLiquidity
        // unreachability witness: patch the slot straight into the wire bytes,
        // because a padding action's encoder refuses to produce it.
        let mut patched = bytes;
        patched[generated::REQUEST_NOW_OFFSET..generated::REQUEST_NOW_OFFSET + 8]
            .copy_from_slice(&7_u64.to_le_bytes());
        match action.now_discipline() {
            NowDisciplineV1::CanonicalZero => {
                assert_eq!(
                    Request::decode(&patched),
                    Err(Error::NonCanonicalPadding),
                    "{action:?} reads no slot, so a nonzero now is padding"
                );
                assert_eq!(
                    Request {
                        now: 7,
                        ..canonical
                    }
                    .to_bytes(),
                    Err(Error::NonCanonicalPadding),
                    "{action:?} must refuse in the encoder too"
                );
            }
            NowDisciplineV1::ExecutionSlot => {
                assert_eq!(
                    Request::decode(&patched).map(|request| request.now),
                    Ok(7),
                    "{action:?} consumes the slot, so the shape may not pin it"
                );
            }
        }
    }
}

/// One shape-canonical request per action at the shared initial state.
fn shape_canonical(action: Action) -> Request {
    let base = base_request(action, initial_state());
    match action {
        Action::ScheduleReplacement => Request {
            actor_id: policy().dealer_id,
            replacement_candidate_id: id(12),
            ..base
        },
        Action::ActivateReplacement => Request {
            replacement_candidate_id: id(12),
            ..base
        },
        Action::Fill | Action::Unwind => Request {
            quantity: 10,
            ..base
        },
        Action::EnterTerminal => Request {
            actor_id: policy().market_id,
            ..base
        },
        Action::Retire => base,
        Action::AddLiquidity | Action::RemoveLiquidity => Request {
            quantity: 10,
            actor_id: policy().dealer_id,
            ..base
        },
    }
}
