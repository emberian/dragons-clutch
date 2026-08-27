//! Host package for the real-ELF Dealer wave campaign.

#[cfg(test)]
mod tests {
    use dclutch_dealer_codec::{
        Action, CANDIDATE_BYTES, CandidateInput, CurveBand, CurveInput, MAX_OUTCOMES,
        NowDisciplineV1, Phase, Policy, Request, Side, State, encode_candidate,
    };
    use solana_account::Account;
    use solana_instruction::{AccountMeta, Instruction};
    use solana_keypair::Keypair;
    use solana_program_test::{ProgramTest, tokio};
    use solana_pubkey::Pubkey;
    use solana_signer::Signer;
    use solana_transaction::Transaction;

    const PROGRAM_ID: Pubkey = Pubkey::new_from_array([40; 32]);
    const POLICY: Pubkey = Pubkey::new_from_array([41; 32]);
    const CANDIDATE: Pubkey = Pubkey::new_from_array([42; 32]);
    const STATE: Pubkey = Pubkey::new_from_array([43; 32]);
    const REPLACEMENT: Pubkey = Pubkey::new_from_array([44; 32]);

    fn policy() -> Policy {
        Policy {
            market_id: [1; 32],
            release_set_id: [2; 32],
            dealer_id: [3; 32],
            fee_recipient_id: [4; 32],
            unwind_recipient_id: [5; 32],
            outcome_count: 3,
            quote_scale: 100,
            fee_numerator: 1,
            fee_denominator: 100,
            minimum_work_funding: 50,
            replacement_delay: 5,
        }
    }

    fn candidate() -> [u8; CANDIDATE_BYTES] {
        candidate_with([6; 32], 1, 0)
    }

    fn candidate_with(
        candidate_id: [u8; 32],
        revision: u64,
        valid_from: u64,
    ) -> [u8; CANDIDATE_BYTES] {
        let bids = [CurveBand {
            capacity: 100,
            price_numerator: 40,
        }];
        let asks = [CurveBand {
            capacity: 100,
            price_numerator: 60,
        }];
        let curves = [CurveInput {
            bids: &bids,
            asks: &asks,
        }; 3];
        let mut output = [0_u8; CANDIDATE_BYTES];
        encode_candidate(
            &mut output,
            CandidateInput {
                candidate_id,
                revision,
                valid_from,
                expires_at: 1_000,
                quote_reserve_floor: 100,
                work_funding: 100,
                work_reward: 2,
                minimum_inventory: &[0, 0, 0],
                maximum_inventory: &[100, 100, 100],
                curves: &curves,
            },
        )
        .expect("candidate");
        output
    }

    fn initial_state() -> State {
        let mut inventory = [0; MAX_OUTCOMES];
        inventory[..3].copy_from_slice(&[50, 50, 50]);
        State {
            phase: Phase::Open,
            outcome_count: 3,
            winner: 0,
            active_candidate_id: [6; 32],
            pending_candidate_id: [0; 32],
            release_set_id: [2; 32],
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

    fn request(action: Action, state: State) -> Request {
        Request {
            action,
            side: Side::TakerBuys,
            outcome: 0,
            expected_state_revision: state.state_revision,
            // Derived, never stamped: the five actions whose transition reads
            // no slot must encode `now == 0`. See `Action::now_discipline`.
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

    fn instruction(
        request: Request,
        active_candidate: Pubkey,
        auxiliary_candidate: Option<Pubkey>,
    ) -> Instruction {
        let mut accounts = vec![
            AccountMeta::new_readonly(POLICY, false),
            AccountMeta::new_readonly(active_candidate, false),
        ];
        if let Some(auxiliary) = auxiliary_candidate {
            accounts.push(AccountMeta::new_readonly(auxiliary, false));
        }
        accounts.push(AccountMeta::new(STATE, false));
        Instruction {
            program_id: PROGRAM_ID,
            accounts,
            data: request.to_bytes().expect("request").to_vec(),
        }
    }

    async fn send(
        banks: &mut solana_program_test::BanksClient,
        payer: &Keypair,
        request: Request,
        active_candidate: Pubkey,
        auxiliary_candidate: Option<Pubkey>,
    ) -> Result<(), solana_program_test::BanksClientError> {
        let blockhash = banks.get_latest_blockhash().await.expect("blockhash");
        let transaction = Transaction::new_signed_with_payer(
            &[instruction(request, active_candidate, auxiliary_candidate)],
            Some(&payer.pubkey()),
            &[payer],
            blockhash,
        );
        banks.process_transaction(transaction).await
    }

    async fn state(banks: &mut solana_program_test::BanksClient) -> State {
        let account = banks
            .get_account(STATE)
            .await
            .expect("get state")
            .expect("state");
        State::decode(&account.data).expect("state bytes")
    }

    fn program_account(data: Vec<u8>) -> Account {
        Account {
            lamports: 1_000_000,
            data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        }
    }

    #[tokio::test]
    async fn real_elf_executes_fill_liquidity_and_atomic_stale_refusal() {
        let mut test = ProgramTest::new("dclutch_trading_dealer_wave_fixture", PROGRAM_ID, None);
        test.add_account(
            POLICY,
            program_account(policy().to_bytes().expect("policy").to_vec()),
        );
        test.add_account(CANDIDATE, program_account(candidate().to_vec()));
        test.add_account(
            REPLACEMENT,
            program_account(candidate_with([7; 32], 2, 20).to_vec()),
        );
        test.add_account(
            STATE,
            program_account(initial_state().to_bytes().expect("state").to_vec()),
        );
        let (mut banks, payer, _) = test.start().await;

        let initial = initial_state();
        send(
            &mut banks,
            &payer,
            Request {
                quantity: 10,
                ..request(Action::Fill, initial)
            },
            CANDIDATE,
            None,
        )
        .await
        .expect("real ELF fill");
        let account = banks
            .get_account(STATE)
            .await
            .expect("get state")
            .expect("state");
        let filled = State::decode(&account.data).expect("filled state");
        assert_eq!(filled.inventory[..3], [40, 50, 50]);
        assert_eq!((filled.quote_custody, filled.fee_custody), (1_006, 1));

        send(
            &mut banks,
            &payer,
            Request {
                action: Action::AddLiquidity,
                outcome: policy().outcome_count,
                now: 0,
                quantity: 100,
                actor_id: policy().dealer_id,
                ..request(Action::AddLiquidity, filled)
            },
            CANDIDATE,
            None,
        )
        .await
        .expect("real ELF liquidity add");
        let account = banks
            .get_account(STATE)
            .await
            .expect("get state")
            .expect("state");
        let funded = State::decode(&account.data).expect("funded state");
        assert_eq!(funded.quote_custody, 1_106);

        send(
            &mut banks,
            &payer,
            Request {
                action: Action::RemoveLiquidity,
                outcome: policy().outcome_count,
                now: 0,
                quantity: 100,
                actor_id: policy().dealer_id,
                ..request(Action::RemoveLiquidity, funded)
            },
            CANDIDATE,
            None,
        )
        .await
        .expect("real ELF liquidity remove");
        let removed = state(&mut banks).await;
        assert_eq!(removed.quote_custody, 1_006);

        let before = banks
            .get_account(STATE)
            .await
            .expect("get state")
            .expect("state")
            .data;
        let stale = Request {
            expected_state_revision: 1,
            quantity: 1,
            ..request(Action::Fill, removed)
        };
        assert!(
            send(&mut banks, &payer, stale, CANDIDATE, None)
                .await
                .is_err()
        );
        let after = banks
            .get_account(STATE)
            .await
            .expect("get state")
            .expect("state");
        assert_eq!(
            after.data, before,
            "late refusal must roll back byte-for-byte"
        );
    }

    #[tokio::test]
    async fn real_elf_executes_replacement_terminal_unwind_and_retirement() {
        let mut test = ProgramTest::new("dclutch_trading_dealer_wave_fixture", PROGRAM_ID, None);
        test.add_account(
            POLICY,
            program_account(policy().to_bytes().expect("policy").to_vec()),
        );
        test.add_account(CANDIDATE, program_account(candidate().to_vec()));
        test.add_account(
            REPLACEMENT,
            program_account(candidate_with([7; 32], 2, 20).to_vec()),
        );
        test.add_account(
            STATE,
            program_account(initial_state().to_bytes().expect("state").to_vec()),
        );
        let (mut banks, payer, _) = test.start().await;

        let initial = initial_state();
        send(
            &mut banks,
            &payer,
            Request {
                action: Action::ScheduleReplacement,
                now: 10,
                actor_id: policy().dealer_id,
                replacement_candidate_id: [7; 32],
                ..request(Action::ScheduleReplacement, initial)
            },
            CANDIDATE,
            Some(REPLACEMENT),
        )
        .await
        .expect("real ELF schedule replacement");
        let scheduled = state(&mut banks).await;
        assert_eq!(scheduled.pending_candidate_id, [7; 32]);
        assert_eq!(scheduled.liveness_custody, 200);

        send(
            &mut banks,
            &payer,
            Request {
                action: Action::ActivateReplacement,
                now: 20,
                replacement_candidate_id: [7; 32],
                ..request(Action::ActivateReplacement, scheduled)
            },
            CANDIDATE,
            Some(REPLACEMENT),
        )
        .await
        .expect("real ELF activate replacement");
        let activated = state(&mut banks).await;
        assert_eq!(activated.active_candidate_id, [7; 32]);
        assert_eq!(activated.pending_candidate_id, [0; 32]);
        assert_eq!(activated.liveness_custody, 100);

        send(
            &mut banks,
            &payer,
            Request {
                action: Action::EnterTerminal,
                outcome: 0,
                actor_id: policy().market_id,
                ..request(Action::EnterTerminal, activated)
            },
            REPLACEMENT,
            None,
        )
        .await
        .expect("real ELF enter terminal");
        let mut terminal = state(&mut banks).await;
        assert_eq!(terminal.phase, Phase::Terminal);

        for (outcome, quantity) in [(0, 50), (1, 50), (2, 50)] {
            send(
                &mut banks,
                &payer,
                Request {
                    action: Action::Unwind,
                    outcome,
                    quantity,
                    ..request(Action::Unwind, terminal)
                },
                REPLACEMENT,
                None,
            )
            .await
            .expect("real ELF unwind");
            terminal = state(&mut banks).await;
        }
        assert_eq!(terminal.inventory[..3], [0, 0, 0]);
        assert_eq!(terminal.quote_custody, 1_050);

        send(
            &mut banks,
            &payer,
            Request {
                now: 0,
                ..request(Action::Retire, terminal)
            },
            REPLACEMENT,
            None,
        )
        .await
        .expect("real ELF retire");
        let retired = state(&mut banks).await;
        assert_eq!(retired.phase, Phase::Retired);
        assert_eq!(retired.quote_custody, 0);
        assert_eq!(retired.fee_custody, 0);
        assert_eq!(retired.liveness_custody, 0);
    }
}
