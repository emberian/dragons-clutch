//! Real-SBF promotion campaign for resumable occupation resolution.

use {
    clutch_kernel::{BasisMode, PayoutSet, PayoutVector, MAX_PAYOUTS},
    clutch_sbf::{
        instructions::{genesis::RentParameters, observe_resolve, resolution_work},
        seeds,
    },
    clutch_solana_layout::{
        account_len, canonical_outcome_id,
        occupation_resolution::{
            OccupationResolutionAccount, OCCUPATION_RESOLUTION_LEN,
            STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07,
        },
        resolution_work::{
            AbortResolutionWorkV1, BeginResolutionWorkV1, FinalizeResolutionWorkV1,
            FoldResolutionWorkV1, ResolutionWorkAccountV1, FINALIZATION_LARGEST_REMAINDER_V1,
            RESOLUTION_WORK_ACCOUNT_BYTES,
        },
        Hash32, HoardAccount, Intent, MarketAccount, PayoutVectorBytes, PositionAccount,
        SupplyLedgerAccount, TermsAccount, MAX_KNOTS, MAX_OUTCOMES, PAYOUT_INDEX_UNRESOLVED,
        PAYOUT_MAP_UNUSED,
    },
    clutch_solana_reference::KernelAccount,
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, layout_request, outcome_mint_bytes,
        rewrite_plane_source_archive_span, GenesisAccount, Mode, Pda, Plane, COMPUTE_BUDGET,
        MARKET_NONCE, PROGRAM_ID, RENT_SYSVAR, START_BUCKET, SYSTEM_PROGRAM, TOKEN_2022,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_rent::Rent,
    solana_signer::Signer,
    solana_system_interface::{instruction as system_instruction, program as system_program},
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const CLOCK_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
const OUTCOMES: u8 = 4;
const DENOMINATOR: u64 = 64;
const SETS: u64 = 64;
const SPAN: u64 = 3;
// A standalone System account created by transfer must itself be rent exempt.
const WORK_DONATION: u64 = 1_000_000;
const RESERVE_DONATION: u64 = 1_100_000;
const POST_WORK_DONATION: u64 = 1_200_000;
const POST_RESERVE_DONATION: u64 = 1_300_000;
const SUBSTITUTE_ARCHIVE: Address = Address::new_from_array([0x91; 32]);
const SUBSTITUTE_SINK: Address = Address::new_from_array([0x92; 32]);
const INCINERATOR: Address = Address::new_from_array([
    0, 51, 144, 114, 141, 52, 17, 96, 121, 189, 201, 17, 191, 255, 0, 219, 212, 77, 46, 205, 204,
    247, 156, 166, 225, 0, 56, 225, 0, 0, 0, 0,
]);

fn actor_keypair() -> Keypair {
    Keypair::new_from_array([0x41; 32])
}

fn worker_keypair() -> Keypair {
    Keypair::new_from_array([0x42; 32])
}

fn keeper_keypair() -> Keypair {
    Keypair::new_from_array([0x43; 32])
}

fn collateral_mint() -> Address {
    Address::new_from_array([0x6c; 32])
}

fn derive(parts: &[&[u8]]) -> Pda {
    let (address, bump) = Address::find_program_address(parts, &PROGRAM_ID);
    Pda { address, bump }
}

fn encode<F, E>(len: usize, encoder: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, E>,
    E: core::fmt::Debug,
{
    let mut out = vec![0_u8; len];
    assert_eq!(encoder(&mut out).expect("fixture encodes"), len);
    out
}

fn account_mut(plane: &mut Plane, address: Address) -> &mut GenesisAccount {
    plane
        .accounts
        .iter_mut()
        .find(|account| account.address == address)
        .expect("fixture account exists")
}

fn one_hot_payouts() -> ([PayoutVectorBytes; MAX_PAYOUTS], PayoutSet) {
    let mut bytes = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut kernel = [PayoutVector::ZERO; MAX_PAYOUTS];
    for outcome in 0..usize::from(OUTCOMES) {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[outcome] = DENOMINATOR;
        bytes[outcome] = PayoutVectorBytes {
            denominator: DENOMINATOR,
            weights,
        };
        kernel[outcome] = PayoutVector::new(DENOMINATOR, weights);
    }
    (bytes, PayoutSet::new(OUTCOMES, OUTCOMES, kernel))
}

fn occupation_plane(actor: Address, degree: u8, span: u64) -> Plane {
    let mut plane = build_plane(actor, collateral_mint(), MARKET_NONCE, Mode::Funded);
    let old_terms = plane.terms.address;
    let market_address = plane.market.address;
    let position_address = plane.position.address;
    let kernel_address = plane.kernel.address;
    let supply_address = plane.supply.address;
    let hoard_address = plane.hoard.address;
    let resolution_address = plane.resolution.address;
    let (payouts, payout_set) = one_hot_payouts();

    let mut terms = TermsAccount::decode(&account_mut(&mut plane, old_terms).data).unwrap();
    terms.outcome_count = OUTCOMES;
    terms.payout_count = OUTCOMES;
    terms.payouts = payouts;
    terms.basis_degree = degree;
    terms.knot_count = OUTCOMES + 1 - degree;
    terms.uniform_log2_spacing = 3;
    terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    terms.knots = [0; MAX_KNOTS];
    for (index, knot) in terms
        .knots
        .iter_mut()
        .take(usize::from(terms.knot_count))
        .enumerate()
    {
        *knot = (index as u128) * 8;
    }
    terms.statistic_id = STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07;
    terms.expected_end_bucket_exclusive = START_BUCKET + span;
    terms.terms = Hash32::ZERO;
    terms.terms = terms.recomputed_terms_digest().unwrap();
    let terms_pda = derive(&[
        seeds::SEED_TERMS,
        &plane.realm_id.bytes(),
        &terms.terms.bytes(),
    ]);
    terms.stored_bump = terms_pda.bump;
    let terms_account = account_mut(&mut plane, old_terms);
    terms_account.address = terms_pda.address;
    terms_account.data = encode(account_len::TERMS, |out| terms.encode(out));
    plane.terms = terms_pda;
    plane.terms_id = terms.terms;

    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    let market_seed = plane.market_id.bytes();
    plane.outcome_mints.clear();
    for outcome in 0..OUTCOMES {
        outcomes[usize::from(outcome)] = canonical_outcome_id(plane.market_id, outcome);
        plane.outcome_mints.push(derive(&[
            seeds::SEED_OUTCOME_MINT,
            &market_seed,
            &[outcome],
        ]));
    }
    let mut market = MarketAccount::decode(&account_mut(&mut plane, market_address).data).unwrap();
    market.terms = terms.terms;
    market.outcome_count = OUTCOMES;
    market.outcomes = outcomes;
    account_mut(&mut plane, market_address).data =
        encode(account_len::MARKET, |out| market.encode(out));

    let mut internal = [0_u64; MAX_OUTCOMES];
    internal[..usize::from(OUTCOMES)].fill(SETS);
    let mut position =
        PositionAccount::decode(&account_mut(&mut plane, position_address).data).unwrap();
    position.internal = internal;
    account_mut(&mut plane, position_address).data =
        encode(account_len::POSITION, |out| position.encode(out));
    let mut total_supply = [0_u64; MAX_OUTCOMES];
    total_supply[..usize::from(OUTCOMES)].fill(SETS);
    let kernel = KernelAccount {
        market: plane.market_id,
        phase: 0,
        basis_mode: BasisMode::DerivedBasis,
        resolved_payout: 0,
        payouts: payout_set,
        total_supply,
    };
    account_mut(&mut plane, kernel_address).data =
        encode(clutch_solana_reference::KERNEL_ACCOUNT_LEN, |out| {
            kernel.encode(out)
        });
    let mut supply =
        SupplyLedgerAccount::decode(&account_mut(&mut plane, supply_address).data).unwrap();
    supply.outcome_count = OUTCOMES;
    supply.internal_supply = internal;
    supply.external_supply = [0; MAX_OUTCOMES];
    account_mut(&mut plane, supply_address).data =
        encode(account_len::SUPPLY_LEDGER, |out| supply.encode(out));
    let mut hoard = HoardAccount::decode(&account_mut(&mut plane, hoard_address).data).unwrap();
    hoard.collateral_atoms = SETS;
    account_mut(&mut plane, hoard_address).data =
        encode(account_len::HOARD, |out| hoard.encode(out));
    let unresolved = OccupationResolutionAccount::unresolved(
        plane.market_id,
        terms.terms,
        plane.feed_id,
        plane.resolution.bump,
    );
    account_mut(&mut plane, resolution_address).data =
        encode(OCCUPATION_RESOLUTION_LEN, |out| unresolved.encode(out));
    rewrite_plane_source_archive_span(&mut plane, 4, 0, span);
    plane.hoard_atoms = SETS;
    plane
}

struct Scenario {
    bank: ProgramTestContext,
    actor: Keypair,
    worker: Keypair,
    keeper: Keypair,
    plane: Plane,
    work: Address,
    reserve: Address,
    deposit: u64,
    work_rent: u64,
    cost_digest: [u8; 32],
}

impl Scenario {
    async fn start(degree: u8, span: u64, malformed_spec_bump: bool) -> Self {
        assert_eq!(
            resolution_work::RESOLUTION_WORK_NEUTRAL_SINK_V1.to_bytes(),
            INCINERATOR.to_bytes()
        );
        let actor = actor_keypair();
        let worker = worker_keypair();
        let keeper = keeper_keypair();
        let mut plane = occupation_plane(actor.pubkey(), degree, span);
        let source_spec_address = plane.source_spec.address;
        let source_archive_address = plane.source_archive.address;
        if malformed_spec_bump {
            account_mut(&mut plane, source_spec_address).data[290] ^= 1;
        }
        let archive_bytes = account_mut(&mut plane, source_archive_address).data.clone();
        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);
        for wallet in [actor.pubkey(), worker.pubkey(), keeper.pubkey()] {
            test.add_account(
                wallet,
                Account {
                    lamports: 10_000_000_000,
                    data: Vec::new(),
                    owner: system_program::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            );
        }
        for account in &plane.accounts {
            test.add_account(
                account.address,
                Account {
                    lamports: Rent::default().minimum_balance(account.data.len()).max(1),
                    data: account.data.clone(),
                    owner: account.owner,
                    executable: false,
                    rent_epoch: 0,
                },
            );
        }
        test.add_account(
            SUBSTITUTE_ARCHIVE,
            Account {
                lamports: Rent::default().minimum_balance(archive_bytes.len()).max(1),
                data: archive_bytes,
                owner: PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        for address in [INCINERATOR, SUBSTITUTE_SINK] {
            test.add_account(
                address,
                Account {
                    lamports: 1,
                    data: Vec::new(),
                    owner: system_program::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            );
        }
        for mint in &plane.outcome_mints {
            let data = outcome_mint_bytes(plane.market.address, 0);
            test.add_account(
                mint.address,
                Account {
                    lamports: Rent::default().minimum_balance(data.len()).max(1),
                    data,
                    owner: TOKEN_2022,
                    executable: false,
                    rent_epoch: 0,
                },
            );
        }
        let bank = test.start_with_context().await;
        let rent = bank.banks_client.get_rent().await.unwrap();
        #[allow(deprecated)]
        let exemption_threshold = f64::from_le_bytes(rent.exemption_threshold);
        let rent_parameters = RentParameters {
            lamports_per_byte_year: rent.lamports_per_byte,
            exemption_threshold,
        };
        let costs = resolution_work::release_cost_schedule_v1(&rent_parameters).unwrap();
        let deposit = costs.minimum_deposit(span as u8).unwrap();
        let cost_digest = resolution_work::release_cost_schedule_digest_v1(costs);
        let work = derive(&[seeds::SEED_RESOLUTION_WORK, &plane.market_id.bytes()]).address;
        let reserve = derive(&[
            seeds::SEED_RESOLUTION_RESERVE,
            &plane.market_id.bytes(),
            &work.to_bytes(),
        ])
        .address;
        Self {
            bank,
            actor,
            worker,
            keeper,
            plane,
            work,
            reserve,
            deposit,
            work_rent: rent.minimum_balance(RESOLUTION_WORK_ACCOUNT_BYTES),
            cost_digest,
        }
    }

    fn begin(&self, nonce: u8, expires_slot: u64) -> Instruction {
        self.begin_with_deposit(nonce, expires_slot, self.deposit)
    }

    fn begin_with_deposit(
        &self,
        nonce: u8,
        expires_slot: u64,
        declared_deposit: u64,
    ) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::BeginResolutionWork(BeginResolutionWorkV1 {
                    work_nonce: [nonce; 32],
                    finalization_mode: FINALIZATION_LARGEST_REMAINDER_V1,
                    expires_slot,
                    declared_deposit,
                    cost_schedule_digest: self.cost_digest,
                }),
            ),
            vec![
                AccountMeta::new(self.actor.pubkey(), true),
                AccountMeta::new_readonly(self.plane.market.address, false),
                AccountMeta::new_readonly(self.plane.terms.address, false),
                AccountMeta::new_readonly(self.plane.resolution.address, false),
                AccountMeta::new_readonly(self.plane.source_spec.address, false),
                AccountMeta::new_readonly(self.plane.source_archive.address, false),
                AccountMeta::new(self.work, false),
                AccountMeta::new(self.reserve, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR, false),
            ],
        )
    }

    fn fold(&self, work: &ResolutionWorkAccountV1, cursor: u64, count: u8) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::FoldResolutionWork(FoldResolutionWorkV1 {
                    work_commitment: work.work_commitment,
                    archive_account: work.archive_account,
                    archive_commitment: work.archive_commitment,
                    expected_cursor: cursor,
                    record_count: count,
                }),
            ),
            vec![
                AccountMeta::new(self.worker.pubkey(), true),
                AccountMeta::new_readonly(self.plane.market.address, false),
                AccountMeta::new_readonly(self.plane.terms.address, false),
                AccountMeta::new_readonly(self.plane.source_spec.address, false),
                AccountMeta::new_readonly(self.plane.source_archive.address, false),
                AccountMeta::new(self.work, false),
                AccountMeta::new(self.reserve, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR, false),
            ],
        )
    }

    fn finalize(&self, work: &ResolutionWorkAccountV1, payer: Address) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(self.keeper.pubkey(), true),
            AccountMeta::new(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(self.plane.kernel.address, false),
            AccountMeta::new(self.plane.supply.address, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new(self.plane.resolution.address, false),
            AccountMeta::new_readonly(self.plane.feed.address, false),
            AccountMeta::new_readonly(self.plane.source_spec.address, false),
            AccountMeta::new_readonly(self.plane.source_archive.address, false),
        ];
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(mint.address, false)),
        );
        metas.extend([
            AccountMeta::new(payer, false),
            AccountMeta::new(self.work, false),
            AccountMeta::new(self.reserve, false),
            AccountMeta::new(INCINERATOR, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ]);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::FinalizeResolutionWork(FinalizeResolutionWorkV1 {
                    work_commitment: work.work_commitment,
                    expected_cursor: work.next_bucket,
                    expected_archive_commitment: work.archive_commitment,
                }),
            ),
            metas,
        )
    }

    fn abort(&self, work: &ResolutionWorkAccountV1, caller: Address) -> Instruction {
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::AbortResolutionWork(AbortResolutionWorkV1 {
                    work_commitment: work.work_commitment,
                    expected_cursor: work.next_bucket,
                    expected_archive_commitment: work.archive_commitment,
                }),
            ),
            vec![
                AccountMeta::new(caller, true),
                AccountMeta::new(self.actor.pubkey(), false),
                AccountMeta::new_readonly(self.plane.market.address, false),
                AccountMeta::new_readonly(self.plane.terms.address, false),
                AccountMeta::new(self.work, false),
                AccountMeta::new(self.reserve, false),
                AccountMeta::new(INCINERATOR, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR, false),
            ],
        )
    }

    fn monolithic(&self) -> Instruction {
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&0_u64.to_le_bytes());
        data.push(1);
        data.push(PAYOUT_INDEX_UNRESOLVED);
        let mut metas = vec![
            AccountMeta::new_readonly(self.actor.pubkey(), true),
            AccountMeta::new(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(self.plane.kernel.address, false),
            AccountMeta::new(self.plane.supply.address, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new(self.plane.resolution.address, false),
            AccountMeta::new_readonly(self.plane.feed.address, false),
            AccountMeta::new_readonly(self.plane.source_spec.address, false),
            AccountMeta::new_readonly(self.plane.source_archive.address, false),
        ];
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(mint.address, false)),
        );
        assert_eq!(
            metas.len(),
            observe_resolve::OCCUPATION_RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    async fn account(&mut self, address: Address) -> Option<Account> {
        self.bank.banks_client.get_account(address).await.unwrap()
    }

    async fn work_state(&mut self) -> ResolutionWorkAccountV1 {
        ResolutionWorkAccountV1::decode(&self.account(self.work).await.unwrap().data).unwrap()
    }
}

fn budget(limit: u32) -> Instruction {
    Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(limit), Vec::new())
}

fn policy_price() -> Instruction {
    let mut data = Vec::with_capacity(9);
    data.push(3);
    data.extend_from_slice(
        &resolution_work::RESOLUTION_WORK_MICROLAMPORTS_PER_CU_CAP_V1.to_le_bytes(),
    );
    Instruction::new_with_bytes(COMPUTE_BUDGET, &data, Vec::new())
}

async fn send(
    scenario: &mut Scenario,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> (Result<(), TransactionError>, u64) {
    send_with_limit(scenario, instructions, signers, 1_400_000).await
}

async fn send_with_limit(
    scenario: &mut Scenario,
    instructions: &[Instruction],
    signers: &[&Keypair],
    limit: u32,
) -> (Result<(), TransactionError>, u64) {
    let blockhash = scenario
        .bank
        .banks_client
        .get_latest_blockhash()
        .await
        .unwrap();
    let mut all = vec![&scenario.bank.payer];
    all.extend_from_slice(signers);
    let mut routed = Vec::with_capacity(instructions.len() + 2);
    routed.push(budget(limit));
    routed.push(policy_price());
    routed.extend_from_slice(instructions);
    let transaction = Transaction::new_signed_with_payer(
        &routed,
        Some(&scenario.bank.payer.pubkey()),
        &all,
        blockhash,
    );
    let outcome = scenario
        .bank
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap();
    (
        outcome.result,
        outcome
            .metadata
            .map(|metadata| metadata.compute_units_consumed)
            .unwrap_or_default(),
    )
}

async fn succeed(
    scenario: &mut Scenario,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> u64 {
    let (result, units) = send(scenario, instructions, signers).await;
    result.expect("transaction succeeds");
    units
}

async fn succeed_with_limit(
    scenario: &mut Scenario,
    instructions: &[Instruction],
    signers: &[&Keypair],
    limit: u32,
) -> u64 {
    let (result, units) = send_with_limit(scenario, instructions, signers, limit).await;
    result.expect("transaction succeeds at selected policy CU limit");
    units
}

async fn snapshot(scenario: &mut Scenario, addresses: &[Address]) -> Vec<Option<Account>> {
    let mut values = Vec::with_capacity(addresses.len());
    for address in addresses {
        values.push(scenario.account(*address).await);
    }
    values
}

#[tokio::test]
async fn begin_fold_finalize_is_prefund_safe_replay_safe_and_byte_exact() {
    let mut staged = Scenario::start(2, SPAN, false).await;
    let actor = staged.actor.insecure_clone();
    let worker = staged.worker.insecure_clone();
    let keeper = staged.keeper.insecure_clone();
    let work_address = staged.work;
    let reserve_address = staged.reserve;
    let initial_actor = staged.account(actor.pubkey()).await.unwrap().lamports;
    succeed(
        &mut staged,
        &[
            system_instruction::transfer(&actor.pubkey(), &work_address, WORK_DONATION),
            system_instruction::transfer(&actor.pubkey(), &reserve_address, RESERVE_DONATION),
        ],
        &[&actor],
    )
    .await;
    let prefund_watch = [work_address, reserve_address, actor.pubkey()];
    let before_underfunded = snapshot(&mut staged, &prefund_watch).await;
    let underfunded = staged.begin_with_deposit(6, 100, staged.deposit - 1);
    assert!(send(&mut staged, &[underfunded], &[&actor])
        .await
        .0
        .is_err());
    assert_eq!(
        snapshot(&mut staged, &prefund_watch).await,
        before_underfunded
    );
    let begin = staged.begin(7, 100);
    let begin_cu = succeed_with_limit(
        &mut staged,
        core::slice::from_ref(&begin),
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = staged.work_state().await;
    assert_eq!(
        work.funding.donation_lamports,
        WORK_DONATION + RESERVE_DONATION
    );
    assert_eq!(
        staged.account(staged.work).await.unwrap().lamports,
        staged.work_rent + WORK_DONATION
    );
    assert_eq!(
        staged.account(staged.reserve).await.unwrap().lamports,
        staged.deposit - staged.work_rent + RESERVE_DONATION
    );
    assert_eq!(
        staged.account(actor.pubkey()).await.unwrap().lamports,
        initial_actor - staged.deposit - WORK_DONATION - RESERVE_DONATION
    );
    let replay_watch = [staged.work, staged.reserve, actor.pubkey()];
    let before_replay = snapshot(&mut staged, &replay_watch).await;
    assert!(send(&mut staged, &[begin], &[&actor]).await.0.is_err());
    assert_eq!(snapshot(&mut staged, &replay_watch).await, before_replay);

    let work_pair = [work_address, reserve_address];
    let before_wrong = snapshot(&mut staged, &work_pair).await;
    let wrong_fold = staged.fold(&work, work.next_bucket + 1, 1);
    assert!(send(&mut staged, &[wrong_fold], &[&worker],)
        .await
        .0
        .is_err());
    assert_eq!(snapshot(&mut staged, &work_pair).await, before_wrong);
    succeed(
        &mut staged,
        &[
            system_instruction::transfer(&actor.pubkey(), &work_address, POST_WORK_DONATION),
            system_instruction::transfer(&actor.pubkey(), &reserve_address, POST_RESERVE_DONATION),
        ],
        &[&actor],
    )
    .await;
    let worker_before = staged.account(worker.pubkey()).await.unwrap().lamports;
    let fold1 = staged.fold(&work, work.next_bucket, 1);
    let fold1_cu = succeed_with_limit(
        &mut staged,
        &[fold1],
        &[&worker],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    assert_eq!(
        staged.account(worker.pubkey()).await.unwrap().lamports,
        worker_before + resolution_work::RESOLUTION_WORK_FOLD_BASE_REWARD_V1
    );
    let work = staged.work_state().await;
    assert_eq!(
        work.funding.donation_lamports,
        WORK_DONATION + RESERVE_DONATION + POST_WORK_DONATION + POST_RESERVE_DONATION
    );
    let early_watch = [
        staged.plane.market.address,
        staged.plane.kernel.address,
        staged.plane.supply.address,
        staged.plane.resolution.address,
        staged.work,
        staged.reserve,
    ];
    let before_early = snapshot(&mut staged, &early_watch).await;
    let early_finalize = staged.finalize(&work, actor.pubkey());
    assert!(send(&mut staged, &[early_finalize], &[&keeper],)
        .await
        .0
        .is_err());
    assert_eq!(snapshot(&mut staged, &early_watch).await, before_early);
    let fold2 = staged.fold(&work, work.next_bucket, 2);
    let fold2_cu = succeed_with_limit(
        &mut staged,
        &[fold2],
        &[&worker],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = staged.work_state().await;
    staged.bank.warp_to_slot(101).unwrap();
    let late_watch = [
        staged.plane.market.address,
        staged.plane.kernel.address,
        staged.plane.supply.address,
        staged.plane.resolution.address,
        staged.work,
        staged.reserve,
        INCINERATOR,
    ];
    let before_expired_complete_abort = snapshot(&mut staged, &late_watch).await;
    let expired_complete_abort = staged.abort(&work, keeper.pubkey());
    assert!(send(&mut staged, &[expired_complete_abort], &[&keeper])
        .await
        .0
        .is_err());
    assert_eq!(
        snapshot(&mut staged, &late_watch).await,
        before_expired_complete_abort
    );
    let before_wrong_sink = snapshot(&mut staged, &late_watch).await;
    let mut wrong_sink = staged.finalize(&work, actor.pubkey());
    let sink_index = wrong_sink.accounts.len() - 2;
    wrong_sink.accounts[sink_index] = AccountMeta::new(SUBSTITUTE_SINK, false);
    assert!(send(&mut staged, &[wrong_sink], &[&keeper])
        .await
        .0
        .is_err());
    assert_eq!(snapshot(&mut staged, &late_watch).await, before_wrong_sink);
    let keeper_before = staged.account(keeper.pubkey()).await.unwrap().lamports;
    let finalize = staged.finalize(&work, actor.pubkey());
    let finalize_cu = succeed_with_limit(
        &mut staged,
        &[finalize],
        &[&keeper],
        resolution_work::RESOLUTION_WORK_FINALIZE_CU_LIMIT_V1,
    )
    .await;
    assert_eq!(
        staged.account(keeper.pubkey()).await.unwrap().lamports,
        keeper_before + resolution_work::RESOLUTION_WORK_FINALIZE_REWARD_V1
    );
    assert!(staged.account(staged.work).await.is_none());
    assert!(staged.account(staged.reserve).await.is_none());
    assert_eq!(
        staged.account(INCINERATOR).await.unwrap().lamports,
        WORK_DONATION + RESERVE_DONATION + POST_WORK_DONATION + POST_RESERVE_DONATION
    );
    assert_eq!(
        staged.account(actor.pubkey()).await.unwrap().lamports,
        initial_actor
            - WORK_DONATION
            - RESERVE_DONATION
            - POST_WORK_DONATION
            - POST_RESERVE_DONATION
            - (2 * resolution_work::RESOLUTION_WORK_FOLD_BASE_REWARD_V1)
            - resolution_work::RESOLUTION_WORK_FINALIZE_REWARD_V1
    );
    let staged_resolution = staged
        .account(staged.plane.resolution.address)
        .await
        .unwrap()
        .data;
    let resolved = OccupationResolutionAccount::decode(&staged_resolution).unwrap();
    assert_eq!(resolved.resolved_slot, 0);

    let mut monolithic = Scenario::start(2, SPAN, false).await;
    let monolithic_actor = monolithic.actor.insecure_clone();
    let monolithic_ix = monolithic.monolithic();
    let monolithic_cu = succeed(&mut monolithic, &[monolithic_ix], &[&monolithic_actor]).await;
    let monolithic_resolution = monolithic
        .account(monolithic.plane.resolution.address)
        .await
        .unwrap()
        .data;
    assert_eq!(staged_resolution, monolithic_resolution);
    for (staged_address, monolithic_address) in [
        (staged.plane.market.address, monolithic.plane.market.address),
        (staged.plane.kernel.address, monolithic.plane.kernel.address),
        (staged.plane.supply.address, monolithic.plane.supply.address),
    ] {
        assert_eq!(
            staged.account(staged_address).await.unwrap().data,
            monolithic.account(monolithic_address).await.unwrap().data
        );
    }
    println!(
        "ResolutionWork SBF CU: Begin={begin_cu} Fold1={fold1_cu} Fold2={fold2_cu} Finalize={finalize_cu} monolithic={monolithic_cu}"
    );
}

#[tokio::test]
async fn expiry_keeper_abort_releases_lock_without_capturing_donations() {
    let mut scenario = Scenario::start(2, SPAN, false).await;
    let actor = scenario.actor.insecure_clone();
    let keeper = scenario.keeper.insecure_clone();
    let work_address = scenario.work;
    let reserve_address = scenario.reserve;
    let initial_actor = scenario.account(actor.pubkey()).await.unwrap().lamports;
    succeed(
        &mut scenario,
        &[
            system_instruction::transfer(&actor.pubkey(), &work_address, WORK_DONATION),
            system_instruction::transfer(&actor.pubkey(), &reserve_address, RESERVE_DONATION),
        ],
        &[&actor],
    )
    .await;
    let begin = scenario.begin(9, 20);
    let begin_cu = succeed_with_limit(
        &mut scenario,
        &[begin],
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = scenario.work_state().await;
    let early_abort = scenario.abort(&work, keeper.pubkey());
    assert!(send(&mut scenario, &[early_abort], &[&keeper],)
        .await
        .0
        .is_err());
    scenario.bank.warp_to_slot(21).unwrap();
    let keeper_before = scenario.account(keeper.pubkey()).await.unwrap().lamports;
    let expired_abort = scenario.abort(&work, keeper.pubkey());
    let abort_cu = succeed_with_limit(
        &mut scenario,
        &[expired_abort],
        &[&keeper],
        resolution_work::RESOLUTION_WORK_ABORT_CU_LIMIT_V1,
    )
    .await;
    assert_eq!(
        scenario.account(keeper.pubkey()).await.unwrap().lamports,
        keeper_before + resolution_work::RESOLUTION_WORK_ABORT_REWARD_V1
    );
    assert!(scenario.account(scenario.work).await.is_none());
    assert!(scenario.account(scenario.reserve).await.is_none());
    assert_eq!(
        scenario.account(INCINERATOR).await.unwrap().lamports,
        WORK_DONATION + RESERVE_DONATION
    );
    assert_eq!(
        scenario.account(actor.pubkey()).await.unwrap().lamports,
        initial_actor
            - WORK_DONATION
            - RESERVE_DONATION
            - resolution_work::RESOLUTION_WORK_ABORT_REWARD_V1
    );
    let reopen = scenario.begin(10, 100);
    let reopen_cu = succeed_with_limit(
        &mut scenario,
        &[reopen],
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    assert_ne!(
        scenario.work_state().await.work_commitment,
        work.work_commitment
    );
    println!(
        "ResolutionWork SBF CU: BeginAbort={begin_cu} AbortExpired={abort_cu} BeginReopen={reopen_cu}"
    );
}

#[tokio::test]
async fn source_spec_bump_and_same_domain_archive_substitution_refuse_atomically() {
    let mut malformed = Scenario::start(2, SPAN, true).await;
    let actor = malformed.actor.insecure_clone();
    let malformed_begin = malformed.begin(11, 100);
    assert!(send(&mut malformed, &[malformed_begin], &[&actor])
        .await
        .0
        .is_err());
    assert!(malformed.account(malformed.work).await.is_none());
    assert!(malformed.account(malformed.reserve).await.is_none());

    let mut scenario = Scenario::start(2, SPAN, false).await;
    let actor = scenario.actor.insecure_clone();
    let worker = scenario.worker.insecure_clone();
    let begin = scenario.begin(12, 100);
    succeed_with_limit(
        &mut scenario,
        &[begin],
        &[&actor],
        resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
    )
    .await;
    let work = scenario.work_state().await;
    let work_pair = [scenario.work, scenario.reserve];
    let before = snapshot(&mut scenario, &work_pair).await;
    let mut wrong = scenario.fold(&work, work.next_bucket, 1);
    wrong.accounts[4] = AccountMeta::new_readonly(SUBSTITUTE_ARCHIVE, false);
    assert!(send(&mut scenario, &[wrong], &[&worker]).await.0.is_err());
    assert_eq!(snapshot(&mut scenario, &work_pair).await, before);
}

#[tokio::test]
async fn every_bounded_fold_size_has_a_real_sbf_cost_row() {
    for record_count in 1_u8..=4 {
        let mut scenario = Scenario::start(2, u64::from(record_count), false).await;
        let actor = scenario.actor.insecure_clone();
        let worker = scenario.worker.insecure_clone();
        let begin = scenario.begin(20 + record_count, 100);
        let begin_cu = succeed_with_limit(
            &mut scenario,
            &[begin],
            &[&actor],
            resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
        )
        .await;
        let work = scenario.work_state().await;
        let fold = scenario.fold(&work, work.next_bucket, record_count);
        let fold_cu = succeed_with_limit(
            &mut scenario,
            &[fold],
            &[&worker],
            resolution_work::RESOLUTION_WORK_FOLD_CU_LIMIT_V1,
        )
        .await;
        assert_eq!(
            scenario.work_state().await.next_bucket,
            work.end_bucket_exclusive
        );
        println!(
            "ResolutionWork SBF CU: span={record_count} Begin={begin_cu} Fold({record_count})={fold_cu}"
        );
    }
}
