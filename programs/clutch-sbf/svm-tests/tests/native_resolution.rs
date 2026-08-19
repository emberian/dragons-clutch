//! Native degree-one through degree-three resolution against the real SBF ELF.
//!
//! These focused scenarios install a version-three Resolution account at
//! genesis so hostile resolved and near-resolved prestates can be constructed
//! directly. Production `CreateMarket` separately selects the 165-byte v2
//! record for degree zero and the 319-byte v3 record for degrees one through
//! three. This campaign isolates the resolution claim: the real program
//! derives the exact vector, persists it once, replays it idempotently, and
//! reconstructs it ephemerally for an exact fractional internal redemption.

use {
    clutch_kernel::{PayoutSet, PayoutVector, MAX_PAYOUTS},
    clutch_sbf::{instructions::observe_resolve, seeds},
    clutch_solana_layout::{
        account_len, canonical_outcome_id,
        native_resolution::{
            NativeResolutionAccount, NATIVE_RESOLUTION_LEN, RESOLUTION_MODE_DERIVED_POINT,
        },
        Hash32, HoardAccount, MarketAccount, PayoutVectorBytes, PositionAccount,
        SupplyLedgerAccount, TermsAccount, MAX_KNOTS, MAX_OUTCOMES, PAYOUT_INDEX_UNRESOLVED,
        PAYOUT_MAP_UNUSED,
    },
    clutch_solana_reference::{KernelAccount, ReplayAccount},
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, immutable_owner_account_bytes, outcome_mint_bytes,
        token_account_bytes, GenesisAccount, Mode, Pda, Plane, BUFFER_ACCOUNT, CASH_ATOMS,
        COMPUTE_BUDGET, MARKET_NONCE, PROGRAM_ID, TOKEN_2022,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, BanksClient, ProgramTest},
    solana_rent::Rent,
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const OUTCOMES: u8 = 4;
const DENOMINATOR: u64 = 64;
const SETS: u64 = 64;
const EMPTY_BUFFER: Address = Address::new_from_array([0x8d; 32]);
const ACTOR_TOKEN: Address = Address::new_from_array([0x8e; 32]);
const CONFLICT_BUFFER: Address = Address::new_from_array([0x8f; 32]);

fn actor_keypair() -> Keypair {
    Keypair::new_from_array([
        0x77, 0x19, 0x42, 0xa8, 0x51, 0x0e, 0xf3, 0x22, 0x63, 0x99, 0x14, 0xc0, 0x2d, 0x6b, 0x84,
        0x31, 0x7a, 0x55, 0xd8, 0x0b, 0xe2, 0x40, 0x6f, 0x91, 0x13, 0xcc, 0x75, 0x28, 0x9d, 0x04,
        0xb6, 0x5e,
    ])
}

fn collateral_mint() -> Address {
    Address::new_from_array([0x6c; 32])
}

fn derive(seeds: &[&[u8]]) -> Pda {
    let (address, bump) = Address::find_program_address(seeds, &PROGRAM_ID);
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

fn evidence_buffer(window_id: Hash32, feed: Hash32, low: u128, high: u128) -> Vec<u8> {
    let mut window = vec![0x45_u8, 1];
    window.extend_from_slice(&feed.bytes());
    window.extend_from_slice(&feed.bytes());
    window.extend_from_slice(&1_u32.to_le_bytes());
    window.extend_from_slice(&1_u32.to_le_bytes());
    window.extend_from_slice(&7_u32.to_le_bytes());
    window.extend_from_slice(&1_u16.to_le_bytes());
    window.extend_from_slice(&60_u64.to_le_bytes());
    window.extend_from_slice(&100_u64.to_le_bytes());
    window.extend_from_slice(&103_u64.to_le_bytes());
    window.extend_from_slice(&104_u64.to_le_bytes());
    window.extend_from_slice(&0_u64.to_le_bytes());
    window.extend_from_slice(&1_u16.to_le_bytes());
    window.extend_from_slice(&0_u64.to_le_bytes());
    window.extend_from_slice(&3_u16.to_le_bytes());
    for bucket in 100_u64..103 {
        window.push(1);
        window.extend_from_slice(&bucket.to_le_bytes());
        window.extend_from_slice(&low.to_le_bytes());
        window.extend_from_slice(&high.to_le_bytes());
    }
    let mut out = vec![0_u8; observe_resolve::EVIDENCE_BUFFER_HEADER_BYTES];
    out[0] = observe_resolve::EVIDENCE_BUFFER_TAG;
    out[1] = observe_resolve::BUFFER_VERSION;
    out[2..34].copy_from_slice(&window_id.bytes());
    out[34..36].copy_from_slice(&(window.len() as u16).to_le_bytes());
    out.extend_from_slice(&window);
    out
}

fn empty_evidence_buffer(window_id: Hash32) -> Vec<u8> {
    let mut out = vec![0_u8; observe_resolve::EVIDENCE_BUFFER_HEADER_BYTES];
    out[0] = observe_resolve::EVIDENCE_BUFFER_TAG;
    out[1] = observe_resolve::BUFFER_VERSION;
    out[2..34].copy_from_slice(&window_id.bytes());
    out
}

fn smooth_plane(actor: Address, degree: u8, low: u128, high: u128) -> Plane {
    let mut plane = build_plane(actor, collateral_mint(), MARKET_NONCE, Mode::Funded);
    let old_terms_address = plane.terms.address;
    let market_id = plane.market_id;
    let market_address = plane.market.address;
    let position_address = plane.position.address;
    let kernel_address = plane.kernel.address;
    let supply_address = plane.supply.address;
    let hoard_address = plane.hoard.address;
    let resolution_address = plane.resolution.address;
    let (payout_bytes, payout_set) = one_hot_payouts();

    let mut terms = TermsAccount::decode(&account_mut(&mut plane, old_terms_address).data)
        .expect("base terms decode");
    terms.outcome_count = OUTCOMES;
    terms.payout_count = OUTCOMES;
    terms.payouts = payout_bytes;
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
    terms.terms = Hash32::ZERO;
    terms.terms = terms
        .recomputed_terms_digest()
        .expect("smooth terms digest");
    let realm = plane.realm_id.bytes();
    let terms_id = terms.terms.bytes();
    let terms_pda = derive(&[seeds::SEED_TERMS, &realm, &terms_id]);
    terms.stored_bump = terms_pda.bump;
    let terms_account = account_mut(&mut plane, old_terms_address);
    terms_account.address = terms_pda.address;
    terms_account.data = encode(account_len::TERMS, |out| terms.encode(out));
    plane.terms = terms_pda;
    plane.terms_id = terms.terms;

    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    let market_seed = market_id.bytes();
    plane.outcome_mints.clear();
    for outcome in 0..OUTCOMES {
        outcomes[usize::from(outcome)] = canonical_outcome_id(market_id, outcome);
        plane.outcome_mints.push(derive(&[
            seeds::SEED_OUTCOME_MINT,
            &market_seed,
            &[outcome],
        ]));
    }
    let mut market = MarketAccount::decode(&account_mut(&mut plane, market_address).data)
        .expect("market decodes");
    market.terms = terms.terms;
    market.outcome_count = OUTCOMES;
    market.outcomes = outcomes;
    account_mut(&mut plane, market_address).data =
        encode(account_len::MARKET, |out| market.encode(out));

    let mut internal = [0_u64; MAX_OUTCOMES];
    internal[..usize::from(OUTCOMES)].fill(SETS);
    let mut position = PositionAccount::decode(&account_mut(&mut plane, position_address).data)
        .expect("position decodes");
    position.internal = internal;
    account_mut(&mut plane, position_address).data =
        encode(account_len::POSITION, |out| position.encode(out));

    let kernel = KernelAccount {
        market: market_id,
        phase: 0,
        resolved_payout: 0,
        payouts: payout_set,
        total_supply: internal,
    };
    account_mut(&mut plane, kernel_address).data =
        encode(clutch_solana_reference::KERNEL_ACCOUNT_LEN, |out| {
            kernel.encode(out)
        });
    let mut supply = SupplyLedgerAccount::decode(&account_mut(&mut plane, supply_address).data)
        .expect("supply decodes");
    supply.outcome_count = OUTCOMES;
    supply.internal_supply = internal;
    supply.external_supply = [0; MAX_OUTCOMES];
    account_mut(&mut plane, supply_address).data =
        encode(account_len::SUPPLY_LEDGER, |out| supply.encode(out));
    let mut hoard =
        HoardAccount::decode(&account_mut(&mut plane, hoard_address).data).expect("hoard decodes");
    hoard.collateral_atoms = SETS;
    account_mut(&mut plane, hoard_address).data =
        encode(account_len::HOARD, |out| hoard.encode(out));

    let unresolved = NativeResolutionAccount::unresolved(
        market_id,
        terms.terms,
        plane.feed_id,
        plane.resolution.bump,
    );
    account_mut(&mut plane, resolution_address).data =
        encode(NATIVE_RESOLUTION_LEN, |out| unresolved.encode(out));
    plane.window_id = Hash32::from_bytes([0x70 + degree; 32]);
    account_mut(&mut plane, BUFFER_ACCOUNT).data =
        evidence_buffer(plane.window_id, plane.feed_id, low, high);
    plane.hoard_atoms = SETS;
    plane
}

fn collateral_mint_bytes(supply: u64) -> Vec<u8> {
    let mut out = vec![0_u8; 82];
    out[36..44].copy_from_slice(&supply.to_le_bytes());
    out[44] = 6;
    out[45] = 1;
    out
}

struct Scenario {
    banks: BanksClient,
    payer: Keypair,
    actor: Keypair,
    plane: Plane,
}

impl Scenario {
    async fn start(
        degree: u8,
        low: u128,
        high: u128,
        hostile_supply: Option<(usize, u64)>,
    ) -> Self {
        let actor = actor_keypair();
        let plane = smooth_plane(actor.pubkey(), degree, low, high);
        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);
        test.add_account(
            actor.pubkey(),
            Account {
                lamports: 10_000_000_000,
                data: Vec::new(),
                owner: solana_system_interface::program::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
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
        for (index, mint) in plane.outcome_mints.iter().enumerate() {
            let supply = hostile_supply
                .filter(|(hostile, _)| *hostile == index)
                .map(|(_, supply)| supply)
                .unwrap_or(0);
            let data = outcome_mint_bytes(plane.market.address, supply);
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
        let mint_data = collateral_mint_bytes(SETS + CASH_ATOMS);
        test.add_account(
            collateral_mint(),
            Account {
                lamports: Rent::default().minimum_balance(mint_data.len()).max(1),
                data: mint_data,
                owner: TOKEN_2022,
                executable: false,
                rent_epoch: 0,
            },
        );
        let actor_data = token_account_bytes(collateral_mint(), actor.pubkey(), 0);
        test.add_account(
            ACTOR_TOKEN,
            Account {
                lamports: Rent::default().minimum_balance(actor_data.len()).max(1),
                data: actor_data,
                owner: TOKEN_2022,
                executable: false,
                rent_epoch: 0,
            },
        );
        let hoard_data = immutable_owner_account_bytes(
            collateral_mint(),
            plane.hoard_authority.address,
            SETS + CASH_ATOMS,
        );
        test.add_account(
            plane.hoard_token.address,
            Account {
                lamports: Rent::default().minimum_balance(hoard_data.len()).max(1),
                data: hoard_data,
                owner: TOKEN_2022,
                executable: false,
                rent_epoch: 0,
            },
        );
        let empty = empty_evidence_buffer(plane.window_id);
        test.add_account(
            EMPTY_BUFFER,
            Account {
                lamports: Rent::default().minimum_balance(empty.len()).max(1),
                data: empty,
                owner: PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        let conflict = evidence_buffer(Hash32::from_bytes([0x99; 32]), plane.feed_id, low, high);
        test.add_account(
            CONFLICT_BUFFER,
            Account {
                lamports: Rent::default().minimum_balance(conflict.len()).max(1),
                data: conflict,
                owner: PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        let (banks, payer, _) = test.start().await;
        Self {
            banks,
            payer,
            actor,
            plane,
        }
    }

    fn resolve(&self) -> Instruction {
        self.resolve_from(self.plane.buffer)
    }

    fn resolve_from(&self, buffer: Address) -> Instruction {
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
            AccountMeta::new_readonly(buffer, false),
        ];
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(mint.address, false)),
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    fn redeem(&self, sequence: u64, outcome: u8, quantity: u64) -> Instruction {
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&sequence.to_le_bytes());
        data.push(2);
        data.push(outcome);
        data.extend_from_slice(&quantity.to_le_bytes());
        let mut metas = vec![
            AccountMeta::new_readonly(self.actor.pubkey(), true),
            AccountMeta::new(self.plane.market.address, false),
            AccountMeta::new(self.plane.hoard.address, false),
            AccountMeta::new(self.plane.position.address, false),
            AccountMeta::new(self.plane.kernel.address, false),
            AccountMeta::new(self.plane.replay.address, false),
            AccountMeta::new(self.plane.supply.address, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new_readonly(self.plane.resolution.address, false),
            AccountMeta::new_readonly(self.plane.feed.address, false),
            AccountMeta::new_readonly(EMPTY_BUFFER, false),
            AccountMeta::new_readonly(self.plane.profile.address, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.plane.policy_account, false),
            AccountMeta::new_readonly(collateral_mint(), false),
            AccountMeta::new(ACTOR_TOKEN, false),
            AccountMeta::new_readonly(self.plane.hoard_authority.address, false),
            AccountMeta::new(self.plane.hoard_token.address, false),
        ];
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(mint.address, false)),
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    async fn try_send(&mut self, instruction: Instruction) -> (Result<(), TransactionError>, u64) {
        let blockhash = self.banks.get_latest_blockhash().await.unwrap();
        let budget = Instruction::new_with_bytes(
            COMPUTE_BUDGET,
            &compute_unit_limit_data(1_400_000),
            Vec::new(),
        );
        let transaction = Transaction::new_signed_with_payer(
            &[budget, instruction],
            Some(&self.payer.pubkey()),
            &[&self.payer, &self.actor],
            blockhash,
        );
        let outcome = self
            .banks
            .process_transaction_with_metadata(transaction)
            .await
            .unwrap();
        let units = outcome
            .metadata
            .map(|metadata| metadata.compute_units_consumed)
            .unwrap_or_default();
        (outcome.result, units)
    }

    async fn data(&mut self, address: Address) -> Vec<u8> {
        self.banks
            .get_account(address)
            .await
            .unwrap()
            .expect("account exists")
            .data
    }
}

fn expected_weights(degree: u8) -> [u64; MAX_OUTCOMES] {
    let mut out = [0_u64; MAX_OUTCOMES];
    match degree {
        1 => out[..4].copy_from_slice(&[32, 32, 0, 0]),
        2 => out[..4].copy_from_slice(&[16, 40, 8, 0]),
        3 => out[..4].copy_from_slice(&[8, 24, 24, 8]),
        _ => panic!("test degree"),
    }
    out
}

#[tokio::test]
async fn degrees_one_through_three_persist_retry_and_redeem_the_native_vector() {
    for degree in 1..=3 {
        let mut scenario = Scenario::start(degree, 4, 4, None).await;
        let (result, resolve_units) = scenario.try_send(scenario.resolve()).await;
        result.expect("native resolve succeeds");
        let record_bytes = scenario.data(scenario.plane.resolution.address).await;
        let record = NativeResolutionAccount::decode(&record_bytes).expect("v3 record decodes");
        assert_eq!(record.mode, RESOLUTION_MODE_DERIVED_POINT);
        assert_eq!(record.resolved_value, 4);
        assert_eq!(record.outcome_count, OUTCOMES);
        assert_eq!(record.vector.denominator, DENOMINATOR);
        assert_eq!(record.vector.weights, expected_weights(degree));

        let market_before_retry = scenario.data(scenario.plane.market.address).await;
        let kernel_before_retry = scenario.data(scenario.plane.kernel.address).await;
        let supply_before_retry = scenario.data(scenario.plane.supply.address).await;
        let (retry, retry_units) = scenario.try_send(scenario.resolve()).await;
        retry.expect("exact native retry is idempotent");
        assert_eq!(
            scenario.data(scenario.plane.market.address).await,
            market_before_retry
        );
        assert_eq!(
            scenario.data(scenario.plane.kernel.address).await,
            kernel_before_retry
        );
        assert_eq!(
            scenario.data(scenario.plane.supply.address).await,
            supply_before_retry
        );
        assert_eq!(
            scenario.data(scenario.plane.resolution.address).await,
            record_bytes
        );

        let position_before_remainder = scenario.data(scenario.plane.position.address).await;
        let hoard_before_remainder = scenario.data(scenario.plane.hoard.address).await;
        let replay_before_remainder = scenario.data(scenario.plane.replay.address).await;
        assert!(scenario.try_send(scenario.redeem(0, 0, 1)).await.0.is_err());
        assert_eq!(
            scenario.data(scenario.plane.position.address).await,
            position_before_remainder
        );
        assert_eq!(
            scenario.data(scenario.plane.hoard.address).await,
            hoard_before_remainder
        );
        assert_eq!(
            scenario.data(scenario.plane.replay.address).await,
            replay_before_remainder
        );

        let quantity = match degree {
            1 => 2,
            2 => 4,
            3 => 8,
            _ => unreachable!(),
        };
        let (redeemed, redeem_units) = scenario.try_send(scenario.redeem(0, 0, quantity)).await;
        redeemed.expect("exact fractional payout redeems");
        let position =
            PositionAccount::decode(&scenario.data(scenario.plane.position.address).await)
                .expect("position decodes");
        let hoard = HoardAccount::decode(&scenario.data(scenario.plane.hoard.address).await)
            .expect("hoard decodes");
        let replay = ReplayAccount::decode(&scenario.data(scenario.plane.replay.address).await)
            .expect("replay decodes");
        assert_eq!(position.internal[0], SETS - quantity);
        assert_eq!(position.cash_atoms, CASH_ATOMS + 1);
        assert_eq!(hoard.collateral_atoms, SETS - 1);
        assert_eq!(replay.sequence, 1);
        assert_eq!(
            scenario.data(scenario.plane.resolution.address).await,
            record_bytes
        );
        println!(
            "native d{degree}: resolve {resolve_units} CU, retry {retry_units} CU, redeem {redeem_units} CU"
        );
    }
}

#[tokio::test]
async fn non_point_evidence_and_conflicting_retry_refuse_without_writes() {
    for degree in 1..=3 {
        let mut scenario = Scenario::start(degree, 4, 5, None).await;
        let before = [
            scenario.data(scenario.plane.market.address).await,
            scenario.data(scenario.plane.kernel.address).await,
            scenario.data(scenario.plane.supply.address).await,
            scenario.data(scenario.plane.resolution.address).await,
        ];
        assert!(scenario.try_send(scenario.resolve()).await.0.is_err());
        let after = [
            scenario.data(scenario.plane.market.address).await,
            scenario.data(scenario.plane.kernel.address).await,
            scenario.data(scenario.plane.supply.address).await,
            scenario.data(scenario.plane.resolution.address).await,
        ];
        assert_eq!(after, before, "d{degree} non-point refusal rolled back");
    }

    let mut scenario = Scenario::start(2, 4, 4, None).await;
    scenario
        .try_send(scenario.resolve())
        .await
        .0
        .expect("first resolve");
    let resolved = scenario.data(scenario.plane.resolution.address).await;
    assert!(scenario
        .try_send(scenario.resolve_from(CONFLICT_BUFFER))
        .await
        .0
        .is_err());
    assert_eq!(
        scenario.data(scenario.plane.resolution.address).await,
        resolved
    );
}

#[tokio::test]
async fn a_late_external_supply_overflow_rolls_back_native_resolution() {
    let mut scenario = Scenario::start(3, 4, 4, Some((0, u64::MAX))).await;
    let before = [
        scenario.data(scenario.plane.market.address).await,
        scenario.data(scenario.plane.kernel.address).await,
        scenario.data(scenario.plane.supply.address).await,
        scenario.data(scenario.plane.resolution.address).await,
    ];
    assert!(scenario.try_send(scenario.resolve()).await.0.is_err());
    let after = [
        scenario.data(scenario.plane.market.address).await,
        scenario.data(scenario.plane.kernel.address).await,
        scenario.data(scenario.plane.supply.address).await,
        scenario.data(scenario.plane.resolution.address).await,
    ];
    assert_eq!(after, before);
}

#[tokio::test]
async fn native_account_mutability_alias_and_full_mint_vector_are_fail_closed() {
    let mut scenario = Scenario::start(2, 4, 4, None).await;
    let before = [
        scenario.data(scenario.plane.market.address).await,
        scenario.data(scenario.plane.kernel.address).await,
        scenario.data(scenario.plane.supply.address).await,
        scenario.data(scenario.plane.resolution.address).await,
    ];

    let mut writable_terms = scenario.resolve();
    writable_terms.accounts[5].is_writable = true;
    assert!(scenario.try_send(writable_terms).await.0.is_err());

    let aliased_buffer = scenario.resolve_from(scenario.plane.feed.address);
    assert!(scenario.try_send(aliased_buffer).await.0.is_err());

    let mut missing_mint = scenario.resolve();
    missing_mint.accounts.pop();
    assert!(scenario.try_send(missing_mint).await.0.is_err());

    let after = [
        scenario.data(scenario.plane.market.address).await,
        scenario.data(scenario.plane.kernel.address).await,
        scenario.data(scenario.plane.supply.address).await,
        scenario.data(scenario.plane.resolution.address).await,
    ];
    assert_eq!(after, before);
}
