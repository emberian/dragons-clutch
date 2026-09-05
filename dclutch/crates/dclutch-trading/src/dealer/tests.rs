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
    candidate_with_work(candidate_id, revision, valid_from, 100, 2)
}

/// The canonical fixture candidate with caller-chosen work economics.
///
/// The exit-affordability invariant is sized in `work_reward` per nonzero
/// inventory coordinate, so a case that wants to sit on either side of the
/// reserve has to be able to move the reward rate.
fn candidate_with_work(
    candidate_id: Identity,
    revision: u64,
    valid_from: u64,
    work_funding: u64,
    work_reward: u64,
) -> [u8; CANDIDATE_BYTES] {
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
            work_funding,
            work_reward,
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

/// The terminal state a market must walk out of, built through the real
/// `EnterTerminal` transition rather than hand-stamped.
fn terminal_state(active: &[u8; CANDIDATE_BYTES], work_remaining: u64) -> State {
    let initial = State {
        active_work_remaining: work_remaining,
        liveness_custody: work_remaining,
        ..initial_state()
    };
    execute(
        initial,
        Request {
            actor_id: policy().market_id,
            outcome: 0,
            ..base_request(Action::EnterTerminal, initial)
        },
        active,
        None,
        None,
    )
    .expect("terminal")
    .post
}

/// The wire — not the transition — is what forbids a zero-quantity economic
/// action, and this test exists to pin WHERE the guard lives.
///
/// `DealerLiquidity.lean` states `0 < quantity` three times (`fillAccepts:457`,
/// `unwindAccepts:527`, `liquidityChangeAccepts:559`). Reading only the
/// transition functions makes it look as though the Rust transcribed it once,
/// into `fill` (`lib.rs`, the `gross == 0 || quantity == 0` conjunct), and
/// dropped it for `unwind` and the two liquidity routes. It did not:
/// `Request::validate_shape` carries it for all four, so a zero-quantity packet
/// is refused before any transition sees it.
///
/// That distinction is worth a test because the transition-level absence looks
/// exactly like a live defect. If it were one it would be a serious one: a
/// zero-quantity unwind moves no inventory and redeems nothing, but
/// `Plan::push` no-ops on a zero amount, so the redemption and the hoard
/// transfer would vanish and the `LivenessVault -> Executor` reward would be
/// the only surviving transfer — a paid no-op, repeatable by anyone, draining
/// the vault that funds the walk to zero inventory in a phase where no party
/// can refill it. The wire is the only thing standing between that and the
/// chain, so if this arm of `validate_shape` is ever relaxed, this test is the
/// one that must go red first.
#[test]
fn the_wire_owns_the_zero_quantity_rule_for_every_economic_action() {
    let state = initial_state();
    for action in [
        Action::Fill,
        Action::Unwind,
        Action::AddLiquidity,
        Action::RemoveLiquidity,
    ] {
        let actor_id = match action {
            Action::AddLiquidity | Action::RemoveLiquidity => policy().dealer_id,
            _ => [0; 32],
        };
        let zero = Request {
            action,
            outcome: 0,
            quantity: 0,
            actor_id,
            side: Side::TakerBuys,
            ..base_request(action, state)
        };
        assert_eq!(
            zero.to_bytes().map(|_| ()),
            Err(Error::NonCanonicalPadding),
            "{action:?} encoded a zero quantity"
        );
        // The same request with a real quantity encodes and decodes, so the
        // refusal above is the quantity field and not some other padding rule
        // this helper happened to violate.
        let real = Request {
            quantity: 10,
            ..zero
        };
        let bytes = real.to_bytes().expect("a nonzero quantity encodes");
        assert_eq!(Request::decode(&bytes), Ok(real));
    }
}

/// An `Unwind` retires one whole coordinate, so the terminal walk is exactly
/// one crank per nonzero coordinate.
///
/// Without the equality a caller chooses the walk's length, and a walk whose
/// length its own callers choose cannot be reserved for — which is what the
/// exit-affordability invariant has to be sized against.
#[test]
fn an_unwind_retires_one_whole_coordinate() {
    let active = candidate(id(11), 1, 0);
    let terminal = terminal_state(&active, 100);
    assert_eq!(
        execute(
            terminal,
            Request {
                outcome: 0,
                quantity: 49,
                ..base_request(Action::Unwind, terminal)
            },
            &active,
            None,
            None,
        ),
        Err(Error::InventoryRisk)
    );
}

/// A `Fill` may not spend the reserve its own exit needs.
///
/// The census reads R4 as "a vanished dealer bricks the market". The vanishing
/// is the mild case: `fill` refuses only BELOW one `work_reward`, so the last
/// admitted fill could legitimately leave the vault at zero, `EnterTerminal`
/// carries that residue through unchanged, and the market arrives in Terminal
/// unable to pay for the unwinds that reach zero inventory — with every party
/// present and cooperative.
///
/// The boundary is pinned from both sides at the same fill, because a test that
/// only shows the refusal cannot distinguish a reserve from an off-by-one.
#[test]
fn a_fill_may_not_spend_the_reserve_its_own_exit_needs() {
    let active = candidate(id(11), 1, 0);
    // Inventory is nonzero at both coordinates, so the walk out costs two
    // cranks at `work_reward` 2 — a reserve of exactly 4.
    let fill = |work_remaining: u64| {
        let state = State {
            active_work_remaining: work_remaining,
            liveness_custody: work_remaining,
            ..initial_state()
        };
        execute(
            state,
            Request {
                quantity: 10,
                ..base_request(Action::Fill, state)
            },
            &active,
            None,
            None,
        )
    };

    // Six covers the fill's own crank and leaves the reserve intact.
    let admitted = fill(6).expect("a fill that leaves the exit funded is admitted");
    assert_eq!(admitted.post.active_work_remaining, 4);
    // The fill moved inventory but did not retire a coordinate, so the walk
    // still costs two cranks and four is exactly enough.
    assert_eq!(admitted.post.inventory[..2], [40, 50]);

    // Four leaves two after the fill's own crank, and two cannot pay for two
    // unwinds. The vault has not underflowed and the fill is affordable on the
    // old rule; what it cannot afford is the exit it would leave behind.
    assert_eq!(fill(4), Err(Error::Underfunded));
}

/// A replacement candidate may not price the standing inventory out of its exit.
///
/// `activate` REPLACES the vault rather than adding to it and refunds the
/// outgoing remainder to `DealerOwner`, and a candidate's only funding floor is
/// one crank. So a dealer could schedule a candidate whose reward rate the
/// standing inventory cannot afford, activate it, take the old vault back, and
/// leave a market nobody can close. That is a deliberate, profitable,
/// protocol-admitted brick — not an absence, and no quiescence timeout would
/// catch it, because the dealer is present and acting.
#[test]
fn a_replacement_may_not_price_the_standing_inventory_out_of_its_exit() {
    let active = candidate(id(11), 1, 0);
    let initial = initial_state();
    // Both candidates carry the same funding; only the reward rate differs, so
    // the two cases cannot be told apart by any earlier gate.
    let activate_with = |work_reward: u64| {
        let next = candidate_with_work(id(12), 2, 20, 50, work_reward);
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
        .expect("schedule");
        assert_eq!(scheduled.post.pending_work_funding, 50);
        execute(
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
    };

    // Two coordinates at 20 a crank is 40 against a 50 vault: affordable.
    let admitted = activate_with(20).expect("an affordable replacement activates");
    assert_eq!(admitted.post.active_work_remaining, 50);
    assert_eq!(admitted.post.active_candidate_id, id(12));

    // The same replacement at 30 a crank needs 60 against the same 50 vault.
    // Every earlier gate is satisfied identically; only the exit is unaffordable.
    assert_eq!(activate_with(30), Err(Error::Underfunded));
}

/// The whole point, stated as the walk: a reserve that is exactly enough.
///
/// R4's brick is closed by construction rather than by a force-exit verb. A
/// state holding exactly the reserve — no slack anywhere — can still walk every
/// coordinate to zero and reach `Retire`, and it ends with the vault at zero
/// rather than short.
#[test]
fn the_exact_reserve_walks_the_whole_way_out_and_retires() {
    let active = candidate(id(11), 1, 0);
    // Two nonzero coordinates at `work_reward` 2: the reserve is 4, and this
    // state holds 4 and nothing more.
    let terminal = terminal_state(&active, 4);
    assert_eq!(terminal.active_work_remaining, 4);

    let mut state = terminal;
    for outcome in [0, 1] {
        state = execute(
            state,
            Request {
                outcome,
                quantity: 50,
                ..base_request(Action::Unwind, state)
            },
            &active,
            None,
            None,
        )
        .expect("the reserve funds every coordinate of the walk")
        .post;
    }
    assert_eq!(state.inventory[..2], [0, 0]);
    assert_eq!(state.active_work_remaining, 0);

    let retired = execute(
        state,
        Request {
            now: 0,
            ..base_request(Action::Retire, state)
        },
        &active,
        None,
        None,
    )
    .expect("retire is reachable");
    assert_eq!(retired.post.phase, Phase::Retired);
}
