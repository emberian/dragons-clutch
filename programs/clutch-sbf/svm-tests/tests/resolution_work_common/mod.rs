//! Shared real-SBF ResolutionWork scenario harness.
//!
//! Used by `tests/resolution_work.rs` (per-instruction promotion campaign) and
//! `tests/resolution_work_batch.rs` (multi-fold single-transaction campaign).
//! Every account image comes from the same encoders the program decodes with.
#![allow(dead_code)]

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
        FeedAccount, Hash32, HoardAccount, Intent, MarketAccount, PayoutVectorBytes,
        PositionAccount, SupplyLedgerAccount, TermsAccount, MAX_KNOTS, MAX_OUTCOMES,
        PAYOUT_INDEX_UNRESOLVED, PAYOUT_MAP_UNUSED,
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
    solana_program_test::{ProgramTest, ProgramTestContext},
    solana_rent::Rent,
    solana_signer::Signer,
    solana_system_interface::program as system_program,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

pub const CLOCK_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
pub const OUTCOMES: u8 = 4;
pub const DENOMINATOR: u64 = 64;
pub const SETS: u64 = 64;
pub const SPAN: u64 = 3;
// A standalone System account created by transfer must itself be rent exempt.
pub const WORK_DONATION: u64 = 1_000_000;
pub const RESERVE_DONATION: u64 = 1_100_000;
pub const POST_WORK_DONATION: u64 = 1_200_000;
pub const POST_RESERVE_DONATION: u64 = 1_300_000;
pub const SUBSTITUTE_ARCHIVE: Address = Address::new_from_array([0x91; 32]);
pub const SUBSTITUTE_SINK: Address = Address::new_from_array([0x92; 32]);
pub const INCINERATOR: Address = Address::new_from_array([
    0, 51, 144, 114, 141, 52, 17, 96, 121, 189, 201, 17, 191, 255, 0, 219, 212, 77, 46, 205, 204,
    247, 156, 166, 225, 0, 56, 225, 0, 0, 0, 0,
]);

pub fn actor_keypair() -> Keypair {
    Keypair::new_from_array([0x41; 32])
}

pub fn worker_keypair() -> Keypair {
    Keypair::new_from_array([0x42; 32])
}

pub fn keeper_keypair() -> Keypair {
    Keypair::new_from_array([0x43; 32])
}

pub fn collateral_mint() -> Address {
    Address::new_from_array([0x6c; 32])
}

pub fn derive(parts: &[&[u8]]) -> Pda {
    let (address, bump) = Address::find_program_address(parts, &PROGRAM_ID);
    Pda { address, bump }
}

pub fn encode<F, E>(len: usize, encoder: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, E>,
    E: core::fmt::Debug,
{
    let mut out = vec![0_u8; len];
    assert_eq!(encoder(&mut out).expect("fixture encodes"), len);
    out
}

pub fn account_mut(plane: &mut Plane, address: Address) -> &mut GenesisAccount {
    plane
        .accounts
        .iter_mut()
        .find(|account| account.address == address)
        .expect("fixture account exists")
}

pub fn one_hot_payouts() -> ([PayoutVectorBytes; MAX_PAYOUTS], PayoutSet) {
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

pub fn occupation_plane(actor: Address, degree: u8, span: u64) -> Plane {
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
    // The window may not mature before its exclusive end, and the archive
    // window built by `rewrite_plane_source_archive_span` carries exactly
    // this bound for spans past the historical four-bucket maturity.
    terms.maturity_horizon_buckets = terms.maturity_horizon_buckets.max(span);
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
    // Finalization refuses a candidate whose last ingested bucket is newer
    // than the shared Feed cursor.  The long-span ResolutionWork fixture must
    // advance that cursor along with the authenticated archive it just built.
    let feed_address = plane.feed.address;
    let mut feed = FeedAccount::decode(&account_mut(&mut plane, feed_address).data).unwrap();
    feed.cursor = feed.cursor.max(START_BUCKET + span);
    account_mut(&mut plane, feed_address).data = encode(account_len::FEED, |out| feed.encode(out));
    plane.hoard_atoms = SETS;
    plane
}

pub struct Scenario {
    pub bank: ProgramTestContext,
    pub actor: Keypair,
    pub worker: Keypair,
    pub keeper: Keypair,
    pub plane: Plane,
    pub work: Address,
    pub reserve: Address,
    pub deposit: u64,
    pub work_rent: u64,
    pub cost_digest: [u8; 32],
}

impl Scenario {
    pub async fn start(degree: u8, span: u64, malformed_spec_bump: bool) -> Self {
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

    pub fn begin(&self, nonce: u8, expires_slot: u64) -> Instruction {
        self.begin_with_deposit(nonce, expires_slot, self.deposit)
    }

    pub fn begin_with_deposit(
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

    pub fn fold(&self, work: &ResolutionWorkAccountV1, cursor: u64, count: u8) -> Instruction {
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

    pub fn finalize(&self, work: &ResolutionWorkAccountV1, payer: Address) -> Instruction {
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

    pub fn abort(&self, work: &ResolutionWorkAccountV1, caller: Address) -> Instruction {
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

    pub fn monolithic(&self) -> Instruction {
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

    pub async fn account(&mut self, address: Address) -> Option<Account> {
        self.bank.banks_client.get_account(address).await.unwrap()
    }

    pub async fn work_state(&mut self) -> ResolutionWorkAccountV1 {
        ResolutionWorkAccountV1::decode(&self.account(self.work).await.unwrap().data).unwrap()
    }
}

pub fn budget(limit: u32) -> Instruction {
    Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(limit), Vec::new())
}

pub fn policy_price() -> Instruction {
    let mut data = Vec::with_capacity(9);
    data.push(3);
    data.extend_from_slice(
        &resolution_work::RESOLUTION_WORK_MICROLAMPORTS_PER_CU_CAP_V1.to_le_bytes(),
    );
    Instruction::new_with_bytes(COMPUTE_BUDGET, &data, Vec::new())
}

pub async fn send(
    scenario: &mut Scenario,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> (Result<(), TransactionError>, u64) {
    send_with_limit(scenario, instructions, signers, 1_400_000).await
}

pub async fn send_with_limit(
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

fn short_vec_len(mut value: usize) -> usize {
    let mut bytes = 1;
    while value >= 0x80 {
        value >>= 7;
        bytes += 1;
    }
    bytes
}

/// Send the exact keeper-shaped transaction and return its serialized packet
/// length with the bank CU observation.
///
/// Unlike [`send_with_limit`], the program signer is also the transaction fee
/// payer, so the message needs one signature rather than the test bank payer
/// plus the program signer. This is the shape a keeper can actually submit.
pub async fn send_as_payer_with_limit(
    scenario: &mut Scenario,
    instructions: &[Instruction],
    payer: &Keypair,
    additional_signers: &[&Keypair],
    limit: u32,
) -> (Result<(), TransactionError>, u64, usize) {
    let blockhash = scenario
        .bank
        .banks_client
        .get_latest_blockhash()
        .await
        .unwrap();
    let mut signers = vec![payer];
    signers.extend_from_slice(additional_signers);
    let mut routed = Vec::with_capacity(instructions.len() + 2);
    routed.push(budget(limit));
    routed.push(policy_price());
    routed.extend_from_slice(instructions);
    let transaction =
        Transaction::new_signed_with_payer(&routed, Some(&payer.pubkey()), &signers, blockhash);
    let wire_bytes = short_vec_len(transaction.signatures.len())
        + transaction.signatures.len() * 64
        + transaction.message.serialize().len();
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
        wire_bytes,
    )
}

pub async fn succeed(
    scenario: &mut Scenario,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> u64 {
    let (result, units) = send(scenario, instructions, signers).await;
    result.expect("transaction succeeds");
    units
}

pub async fn succeed_with_limit(
    scenario: &mut Scenario,
    instructions: &[Instruction],
    signers: &[&Keypair],
    limit: u32,
) -> u64 {
    let (result, units) = send_with_limit(scenario, instructions, signers, limit).await;
    result.expect("transaction succeeds at selected policy CU limit");
    units
}

pub async fn snapshot(scenario: &mut Scenario, addresses: &[Address]) -> Vec<Option<Account>> {
    let mut values = Vec::with_capacity(addresses.len());
    for address in addresses {
        values.push(scenario.account(*address).await);
    }
    values
}
