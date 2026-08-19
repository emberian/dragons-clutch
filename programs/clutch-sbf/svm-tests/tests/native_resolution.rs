//! Native degree-one through degree-three point and occupation resolution
//! against the real SBF ELF.
//!
//! These focused scenarios install a version-three Resolution account at
//! genesis so hostile resolved and near-resolved prestates can be constructed
//! directly. Production `CreateMarket` separately selects v2 categorical, v3
//! native point, or v4 native occupation by immutable Terms. This campaign
//! isolates the resolution claim: the real program derives the exact vector,
//! persists it once, replays it idempotently, and reconstructs it ephemerally
//! for exact fractional internal and bearer redemption.

use {
    clutch_kernel::{PayoutSet, PayoutVector, MAX_PAYOUTS},
    clutch_sbf::{instructions::observe_resolve, seeds},
    clutch_solana_layout::{
        account_len, canonical_outcome_id,
        native_resolution::{
            NativeResolutionAccount, NATIVE_RESOLUTION_LEN, RESOLUTION_MODE_DERIVED_POINT,
            RESOLUTION_MODE_PRESET,
        },
        occupation_resolution::{
            OccupationResolutionAccount, OCCUPATION_BASIS_EVALUATOR_VERSION,
            OCCUPATION_FINALIZATION_EXACT_ONLY, OCCUPATION_FINALIZATION_LARGEST_REMAINDER_V1,
            OCCUPATION_RESOLUTION_LEN, OCCUPATION_SUMMARY_VERSION,
            RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
            STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07,
        },
        Hash32, HoardAccount, Intent, MarketAccount, PayoutVectorBytes, PositionAccount,
        SupplyLedgerAccount, TermsAccount, MAX_KNOTS, MAX_OUTCOMES, PAYOUT_INDEX_UNRESOLVED,
        PAYOUT_MAP_UNUSED,
    },
    clutch_solana_reference::{KernelAccount, ReplayAccount},
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, immutable_owner_account_bytes, layout_request,
        outcome_mint_bytes, rewrite_plane_source_archive, rewrite_plane_source_archive_span,
        source_resolution_evidence_buffer, token_account_bytes, GenesisAccount, Mode, Pda, Plane,
        BUFFER_ACCOUNT, CASH_ATOMS, COMPUTE_BUDGET, MARKET_NONCE, PROGRAM_ID, START_BUCKET,
        TOKEN_2022,
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
const ACTOR_TOKEN: Address = Address::new_from_array([0x8e; 32]);
const CONFLICT_BUFFER: Address = Address::new_from_array([0x8f; 32]);
const OUTCOME_SOURCE: Address = Address::new_from_array([0x90; 32]);
const SUBSTITUTE_ARCHIVE: Address = Address::new_from_array([0x91; 32]);

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

fn smooth_plane(actor: Address, degree: u8, low: u128, high: u128) -> Plane {
    smooth_plane_with_external(actor, degree, low, high, 0)
}

fn smooth_plane_with_external(
    actor: Address,
    degree: u8,
    low: u128,
    high: u128,
    external_quantity: u64,
) -> Plane {
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
    internal[0] = internal[0]
        .checked_sub(external_quantity)
        .expect("external fixture is materialized from internal claims");
    let mut position = PositionAccount::decode(&account_mut(&mut plane, position_address).data)
        .expect("position decodes");
    position.internal = internal;
    account_mut(&mut plane, position_address).data =
        encode(account_len::POSITION, |out| position.encode(out));

    let mut total_supply = [0_u64; MAX_OUTCOMES];
    total_supply[..usize::from(OUTCOMES)].fill(SETS);
    let kernel = KernelAccount {
        market: market_id,
        phase: 0,
        basis_mode: clutch_kernel::BasisMode::DerivedBasis,
        resolved_payout: 0,
        payouts: payout_set,
        total_supply,
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
    supply.external_supply[0] = external_quantity;
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
    let width = high.checked_sub(low).expect("ordered fixture interval");
    assert_eq!(width % 2, 0, "source fixture interval must be symmetric");
    let confidence = width / 2;
    let price = low.checked_add(confidence).expect("fixture midpoint");
    rewrite_plane_source_archive(&mut plane, price, confidence);
    account_mut(&mut plane, BUFFER_ACCOUNT).data =
        source_resolution_evidence_buffer(plane.window_id, plane.feed_id, low, high);
    plane.hoard_atoms = SETS;
    plane
}

/// Select the distinct occupation statistic and v4 account without changing
/// the v3 point fixture used by the existing campaign.
fn occupation_plane_with_external(
    actor: Address,
    degree: u8,
    low: u128,
    high: u128,
    external_quantity: u64,
) -> Plane {
    occupation_plane_with_statistic(
        actor,
        degree,
        low,
        high,
        external_quantity,
        STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
    )
}

fn occupation_plane_with_statistic(
    actor: Address,
    degree: u8,
    low: u128,
    high: u128,
    external_quantity: u64,
    statistic: u16,
) -> Plane {
    let mut plane = smooth_plane_with_external(actor, degree, low, high, external_quantity);
    let old_terms_address = plane.terms.address;
    let market_address = plane.market.address;
    let resolution_address = plane.resolution.address;
    let mut terms = TermsAccount::decode(&account_mut(&mut plane, old_terms_address).data)
        .expect("point terms decode");
    terms.statistic_id = statistic;
    terms.terms = Hash32::ZERO;
    terms.terms = terms
        .recomputed_terms_digest()
        .expect("occupation terms digest");
    let realm = plane.realm_id.bytes();
    let terms_id = terms.terms.bytes();
    let terms_pda = derive(&[seeds::SEED_TERMS, &realm, &terms_id]);
    terms.stored_bump = terms_pda.bump;
    let terms_account = account_mut(&mut plane, old_terms_address);
    terms_account.address = terms_pda.address;
    terms_account.data = encode(account_len::TERMS, |out| terms.encode(out));
    plane.terms = terms_pda;
    plane.terms_id = terms.terms;

    let mut market = MarketAccount::decode(&account_mut(&mut plane, market_address).data)
        .expect("market decodes");
    market.terms = terms.terms;
    account_mut(&mut plane, market_address).data =
        encode(account_len::MARKET, |out| market.encode(out));
    let unresolved = OccupationResolutionAccount::unresolved(
        plane.market_id,
        terms.terms,
        plane.feed_id,
        plane.resolution.bump,
    );
    account_mut(&mut plane, resolution_address).data =
        encode(OCCUPATION_RESOLUTION_LEN, |out| unresolved.encode(out));
    plane
}

fn occupation_plane_with_span(actor: Address, degree: u8, span: u64) -> Plane {
    let mut plane = occupation_plane_with_external(actor, degree, 4, 4, 0);
    let old_terms_address = plane.terms.address;
    let market_address = plane.market.address;
    let resolution_address = plane.resolution.address;
    let mut terms = TermsAccount::decode(&account_mut(&mut plane, old_terms_address).data)
        .expect("occupation terms decode");
    terms.expected_end_bucket_exclusive = START_BUCKET
        .checked_add(span)
        .expect("profile span end is representable");
    terms.terms = Hash32::ZERO;
    terms.terms = terms
        .recomputed_terms_digest()
        .expect("span-specific occupation terms digest");
    let realm = plane.realm_id.bytes();
    let terms_id = terms.terms.bytes();
    let terms_pda = derive(&[seeds::SEED_TERMS, &realm, &terms_id]);
    terms.stored_bump = terms_pda.bump;
    let terms_account = account_mut(&mut plane, old_terms_address);
    terms_account.address = terms_pda.address;
    terms_account.data = encode(account_len::TERMS, |out| terms.encode(out));
    plane.terms = terms_pda;
    plane.terms_id = terms.terms;

    let mut market = MarketAccount::decode(&account_mut(&mut plane, market_address).data)
        .expect("market decodes");
    market.terms = terms.terms;
    account_mut(&mut plane, market_address).data =
        encode(account_len::MARKET, |out| market.encode(out));
    let unresolved = OccupationResolutionAccount::unresolved(
        plane.market_id,
        terms.terms,
        plane.feed_id,
        plane.resolution.bump,
    );
    account_mut(&mut plane, resolution_address).data =
        encode(OCCUPATION_RESOLUTION_LEN, |out| unresolved.encode(out));
    rewrite_plane_source_archive_span(&mut plane, 4, 0, span);
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
        Self::start_plane(actor, plane, 0, 0, hostile_supply).await
    }

    async fn start_external(degree: u8, external_quantity: u64, destination_amount: u64) -> Self {
        let actor = actor_keypair();
        let plane = smooth_plane_with_external(actor.pubkey(), degree, 4, 4, external_quantity);
        Self::start_plane(actor, plane, external_quantity, destination_amount, None).await
    }

    async fn start_occupation(
        degree: u8,
        low: u128,
        high: u128,
        external_quantity: u64,
        destination_amount: u64,
        hostile_supply: Option<(usize, u64)>,
    ) -> Self {
        let actor = actor_keypair();
        let plane =
            occupation_plane_with_external(actor.pubkey(), degree, low, high, external_quantity);
        Self::start_plane(
            actor,
            plane,
            external_quantity,
            destination_amount,
            hostile_supply,
        )
        .await
    }

    async fn start_external_resolved(
        degree: u8,
        external_quantity: u64,
        destination_amount: u64,
    ) -> Self {
        let actor = actor_keypair();
        let plane = smooth_plane_with_external(actor.pubkey(), degree, 4, 4, external_quantity);
        let terms = TermsAccount::decode(
            &plane
                .accounts
                .iter()
                .find(|account| account.address == plane.terms.address)
                .expect("terms account")
                .data,
        )
        .expect("terms decode");
        let record = NativeResolutionAccount {
            market: plane.market_id,
            terms: terms.terms,
            feed: terms.feed,
            window: plane.window_id,
            feed_cursor: 104,
            sealed_end_bucket_exclusive: 103,
            repair_generation: 0,
            resolved_slot: 0,
            mode: RESOLUTION_MODE_DERIVED_POINT,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            outcome_count: OUTCOMES,
            resolved_value: 4,
            vector: PayoutVectorBytes {
                denominator: DENOMINATOR,
                weights: expected_weights(degree),
            },
            stored_bump: plane.resolution.bump,
            flags: 0,
        };
        let resolution = encode(NATIVE_RESOLUTION_LEN, |out| record.encode(out));
        Self::start_plane(
            actor,
            force_resolved_plane(plane, resolution),
            external_quantity,
            destination_amount,
            None,
        )
        .await
    }

    async fn start_custom(plane: Plane, external_quantity: u64, destination_amount: u64) -> Self {
        Self::start_plane(
            actor_keypair(),
            plane,
            external_quantity,
            destination_amount,
            None,
        )
        .await
    }

    async fn start_plane(
        actor: Keypair,
        plane: Plane,
        external_quantity: u64,
        destination_amount: u64,
        hostile_supply: Option<(usize, u64)>,
    ) -> Self {
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
        let substitute_archive = plane
            .accounts
            .iter()
            .find(|account| account.address == plane.source_archive.address)
            .expect("source archive account")
            .data
            .clone();
        test.add_account(
            SUBSTITUTE_ARCHIVE,
            Account {
                lamports: Rent::default()
                    .minimum_balance(substitute_archive.len())
                    .max(1),
                data: substitute_archive,
                owner: PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        for (index, mint) in plane.outcome_mints.iter().enumerate() {
            let supply = hostile_supply
                .filter(|(hostile, _)| *hostile == index)
                .map(|(_, supply)| supply)
                .unwrap_or(if index == 0 { external_quantity } else { 0 });
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
        let actor_data = token_account_bytes(collateral_mint(), actor.pubkey(), destination_amount);
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
        let source_data = token_account_bytes(
            plane.outcome_mints[0].address,
            actor.pubkey(),
            external_quantity,
        );
        test.add_account(
            OUTCOME_SOURCE,
            Account {
                lamports: Rent::default().minimum_balance(source_data.len()).max(1),
                data: source_data,
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
        let conflict = source_resolution_evidence_buffer(plane.window_id, plane.feed_id, 3, 5);
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
            AccountMeta::new_readonly(self.plane.source_spec.address, false),
            AccountMeta::new_readonly(self.plane.source_archive.address, false),
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

    fn resolve_occupation(&self) -> Instruction {
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
        assert_eq!(
            metas.len(),
            observe_resolve::REDEEM_ACCOUNT_PREFIX + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    fn external_exit(&self, quantity: u64) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(self.actor.pubkey(), true),
            AccountMeta::new_readonly(self.plane.profile.address, false),
            AccountMeta::new_readonly(self.plane.market.address, false),
            AccountMeta::new(self.plane.hoard.address, false),
            AccountMeta::new(self.plane.kernel.address, false),
            AccountMeta::new(self.plane.supply.address, false),
            AccountMeta::new_readonly(self.plane.resolution.address, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new_readonly(self.plane.policy_account, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(collateral_mint(), false),
            AccountMeta::new(ACTOR_TOKEN, false),
            AccountMeta::new_readonly(self.plane.hoard_authority.address, false),
            AccountMeta::new(self.plane.hoard_token.address, false),
            AccountMeta::new(OUTCOME_SOURCE, false),
        ];
        metas.extend(
            self.plane
                .outcome_mints
                .iter()
                .enumerate()
                .map(|(outcome, mint)| {
                    if outcome == 0 {
                        AccountMeta::new(mint.address, false)
                    } else {
                        AccountMeta::new_readonly(mint.address, false)
                    }
                }),
        );
        let data = layout_request(
            0,
            Intent::RedeemExternal {
                market: self.plane.market_id,
                claimant: Hash32::from_bytes(self.actor.pubkey().to_bytes()),
                source: Hash32::from_bytes(OUTCOME_SOURCE.to_bytes()),
                destination: Hash32::from_bytes(ACTOR_TOKEN.to_bytes()),
                outcome: 0,
                quantity,
            },
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    async fn try_send(&mut self, instruction: Instruction) -> (Result<(), TransactionError>, u64) {
        self.try_send_many(&[instruction]).await
    }

    async fn try_send_many(
        &mut self,
        instructions: &[Instruction],
    ) -> (Result<(), TransactionError>, u64) {
        let blockhash = self.banks.get_latest_blockhash().await.unwrap();
        let budget = Instruction::new_with_bytes(
            COMPUTE_BUDGET,
            &compute_unit_limit_data(1_400_000),
            Vec::new(),
        );
        let mut transaction_instructions = Vec::with_capacity(instructions.len() + 1);
        transaction_instructions.push(budget);
        transaction_instructions.extend_from_slice(instructions);
        let transaction = Transaction::new_signed_with_payer(
            &transaction_instructions,
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
        self.account(address).await.data
    }

    async fn account(&mut self, address: Address) -> Account {
        self.banks
            .get_account(address)
            .await
            .unwrap()
            .expect("account exists")
    }

    async fn token_amount(&mut self, address: Address) -> u64 {
        let data = self.data(address).await;
        u64::from_le_bytes(data[64..72].try_into().expect("token amount field"))
    }

    async fn mint_supply(&mut self, address: Address) -> u64 {
        let data = self.data(address).await;
        u64::from_le_bytes(data[36..44].try_into().expect("mint supply field"))
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

fn exact_lot(degree: u8) -> u64 {
    match degree {
        1 => 2,
        2 => 4,
        3 => 8,
        _ => panic!("test degree"),
    }
}

async fn snapshot(scenario: &mut Scenario, addresses: &[Address]) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(addresses.len());
    for address in addresses {
        out.push(scenario.data(*address).await);
    }
    out
}

fn force_resolved_plane(mut plane: Plane, resolution: Vec<u8>) -> Plane {
    let market_address = plane.market.address;
    let kernel_address = plane.kernel.address;
    let resolution_address = plane.resolution.address;
    let mut market = MarketAccount::decode(&account_mut(&mut plane, market_address).data).unwrap();
    market.lifecycle = 1;
    account_mut(&mut plane, market_address).data =
        encode(account_len::MARKET, |out| market.encode(out));
    let mut kernel = KernelAccount::decode(&account_mut(&mut plane, kernel_address).data).unwrap();
    kernel.phase = 1;
    kernel.resolved_payout = 0;
    account_mut(&mut plane, kernel_address).data =
        encode(clutch_solana_reference::KERNEL_ACCOUNT_LEN, |out| {
            kernel.encode(out)
        });
    account_mut(&mut plane, resolution_address).data = resolution;
    plane
}

fn point_record(plane: &Plane, degree: u8) -> NativeResolutionAccount {
    let terms = TermsAccount::decode(
        &plane
            .accounts
            .iter()
            .find(|account| account.address == plane.terms.address)
            .expect("terms account")
            .data,
    )
    .expect("terms decode");
    NativeResolutionAccount {
        market: plane.market_id,
        terms: terms.terms,
        feed: terms.feed,
        window: plane.window_id,
        feed_cursor: 104,
        sealed_end_bucket_exclusive: 103,
        repair_generation: 0,
        resolved_slot: 0,
        mode: RESOLUTION_MODE_DERIVED_POINT,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        outcome_count: OUTCOMES,
        resolved_value: 4,
        vector: PayoutVectorBytes {
            denominator: DENOMINATOR,
            weights: expected_weights(degree),
        },
        stored_bump: plane.resolution.bump,
        flags: 0,
    }
}

fn occupation_record(
    plane: &Plane,
    degree: u8,
    archive_commitment: Hash32,
) -> OccupationResolutionAccount {
    let terms = TermsAccount::decode(
        &plane
            .accounts
            .iter()
            .find(|account| account.address == plane.terms.address)
            .expect("terms account")
            .data,
    )
    .expect("occupation terms decode");
    let archive = &plane
        .accounts
        .iter()
        .find(|account| account.address == plane.source_archive.address)
        .expect("source archive account")
        .data;
    let u64_at = |offset: usize| {
        u64::from_le_bytes(
            archive[offset..offset + 8]
                .try_into()
                .expect("archive u64 field"),
        )
    };
    OccupationResolutionAccount {
        market: plane.market_id,
        terms: terms.terms,
        feed: terms.feed,
        window: plane.window_id,
        feed_cursor: u64_at(408),
        sealed_end_bucket_exclusive: u64_at(368),
        repair_generation: terms.repair_generation,
        resolved_slot: 0,
        mode: RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        outcome_count: OUTCOMES,
        resolved_value: 0,
        vector: PayoutVectorBytes {
            denominator: DENOMINATOR,
            weights: expected_weights(degree),
        },
        archive_commitment,
        statistic: STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
        finalization: OCCUPATION_FINALIZATION_EXACT_ONLY,
        basis_evaluator_version: OCCUPATION_BASIS_EVALUATOR_VERSION,
        occupation_summary_version: OCCUPATION_SUMMARY_VERSION,
        sample_count: terms.expected_span().expect("bounded span"),
        coverage_count: terms.expected_span().expect("bounded span"),
        gap_count: 0,
        stored_bump: plane.resolution.bump,
        flags: 0,
        reserved: 0,
    }
}

fn source_archive_commitment(plane: &Plane) -> Hash32 {
    let archive = &plane
        .accounts
        .iter()
        .find(|account| account.address == plane.source_archive.address)
        .expect("source archive account")
        .data;
    Hash32::from_bytes(
        archive[472..504]
            .try_into()
            .expect("source archive commitment field"),
    )
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
        let mut scenario = Scenario::start(degree, 3, 5, None).await;
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

    let mut substituted_archive = scenario.resolve();
    substituted_archive.accounts[9] = AccountMeta::new_readonly(SUBSTITUTE_ARCHIVE, false);
    assert!(scenario.try_send(substituted_archive).await.0.is_err());

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

#[tokio::test]
async fn recorded_internal_redemption_rejects_retired_evidence_aliases_and_late_replay_atomically()
{
    let degree = 2;
    let lot = exact_lot(degree);
    let mut scenario = Scenario::start(degree, 4, 4, None).await;
    scenario
        .try_send(scenario.resolve())
        .await
        .0
        .expect("native point resolves before internal redemption");
    let watched = [
        scenario.plane.market.address,
        scenario.plane.hoard.address,
        scenario.plane.position.address,
        scenario.plane.kernel.address,
        scenario.plane.replay.address,
        scenario.plane.supply.address,
        scenario.plane.resolution.address,
        scenario.plane.hoard_token.address,
        ACTOR_TOKEN,
        scenario.plane.outcome_mints[0].address,
        scenario.plane.outcome_mints[1].address,
        scenario.plane.outcome_mints[2].address,
        scenario.plane.outcome_mints[3].address,
    ];
    let before = snapshot(&mut scenario, &watched).await;

    let mut retired = scenario.redeem(0, 0, lot);
    retired.accounts.insert(
        9,
        AccountMeta::new_readonly(scenario.plane.feed.address, false),
    );
    retired
        .accounts
        .insert(10, AccountMeta::new_readonly(scenario.plane.buffer, false));
    assert_eq!(
        retired.accounts.len(),
        observe_resolve::REDEEM_ACCOUNT_PREFIX + usize::from(OUTCOMES) + 2
    );
    assert!(scenario.try_send(retired).await.0.is_err());
    assert_eq!(snapshot(&mut scenario, &watched).await, before);

    let mut aliased_record = scenario.redeem(0, 0, lot);
    aliased_record.accounts[8] = aliased_record.accounts[7].clone();
    assert!(scenario.try_send(aliased_record).await.0.is_err());
    assert_eq!(snapshot(&mut scenario, &watched).await, before);

    let mut incomplete_mints = scenario.redeem(0, 0, lot);
    incomplete_mints.accounts.pop();
    assert!(scenario.try_send(incomplete_mints).await.0.is_err());
    assert_eq!(snapshot(&mut scenario, &watched).await, before);

    let mut writable_record = scenario.redeem(0, 0, lot);
    writable_record.accounts[8].is_writable = true;
    assert!(scenario.try_send(writable_record).await.0.is_err());
    assert_eq!(snapshot(&mut scenario, &watched).await, before);

    let first = scenario.redeem(0, 0, lot);
    let stale_second = scenario.redeem(0, 0, lot);
    assert!(scenario
        .try_send_many(&[first, stale_second])
        .await
        .0
        .is_err());
    assert_eq!(
        snapshot(&mut scenario, &watched).await,
        before,
        "the late replay refusal rolls back the first instruction too"
    );

    scenario
        .try_send(scenario.redeem(0, 0, lot))
        .await
        .0
        .expect("the exact 16+n recorded-resolution plane succeeds");
}

#[tokio::test]
async fn recorded_internal_redemption_rejects_wrong_mode_terms_and_window_without_writes() {
    let degree = 2;
    let lot = exact_lot(degree);
    for corruption in ["mode", "terms", "window"] {
        let actor = actor_keypair();
        let plane = smooth_plane(actor.pubkey(), degree, 4, 4);
        let mut record = point_record(&plane, degree);
        match corruption {
            "mode" => {
                record.mode = RESOLUTION_MODE_PRESET;
                record.payout_index = 0;
                record.outcome_count = 0;
                record.resolved_value = 0;
                record.vector = PayoutVectorBytes::ZERO;
            }
            "terms" => record.terms = Hash32::from_bytes([0xb4; 32]),
            "window" => record.window = Hash32::from_bytes([0xb5; 32]),
            _ => unreachable!(),
        }
        let resolution = encode(NATIVE_RESOLUTION_LEN, |out| record.encode(out));
        let mut scenario =
            Scenario::start_custom(force_resolved_plane(plane, resolution), 0, 0).await;
        let watched = [
            scenario.plane.hoard.address,
            scenario.plane.position.address,
            scenario.plane.kernel.address,
            scenario.plane.replay.address,
            scenario.plane.supply.address,
            scenario.plane.resolution.address,
            scenario.plane.hoard_token.address,
            ACTOR_TOKEN,
        ];
        let before = snapshot(&mut scenario, &watched).await;
        assert!(scenario
            .try_send(scenario.redeem(0, 0, lot))
            .await
            .0
            .is_err());
        assert_eq!(
            snapshot(&mut scenario, &watched).await,
            before,
            "{corruption} corruption changed a writable account"
        );
    }
}

#[tokio::test]
async fn native_bearer_exit_uses_minimal_exact_lots_and_sub_lots_roll_back() {
    for degree in 1..=3 {
        let lot = exact_lot(degree);
        let mut scenario = Scenario::start_external_resolved(degree, lot, 0).await;

        let watched = [
            scenario.plane.hoard.address,
            scenario.plane.kernel.address,
            scenario.plane.supply.address,
            scenario.plane.resolution.address,
            scenario.plane.position.address,
            scenario.plane.hoard_token.address,
            ACTOR_TOKEN,
            OUTCOME_SOURCE,
            scenario.plane.outcome_mints[0].address,
        ];
        let before_sub_lot = snapshot(&mut scenario, &watched).await;
        assert!(scenario
            .try_send(scenario.external_exit(lot - 1))
            .await
            .0
            .is_err());
        assert_eq!(snapshot(&mut scenario, &watched).await, before_sub_lot);

        let resolution_before = scenario.data(scenario.plane.resolution.address).await;
        let position_before = scenario.data(scenario.plane.position.address).await;
        let (result, units) = scenario.try_send(scenario.external_exit(lot)).await;
        result.expect("minimal exact native bearer lot exits");
        assert_eq!(scenario.token_amount(OUTCOME_SOURCE).await, 0);
        assert_eq!(
            scenario
                .mint_supply(scenario.plane.outcome_mints[0].address)
                .await,
            0
        );
        assert_eq!(scenario.token_amount(ACTOR_TOKEN).await, 1);
        assert_eq!(
            scenario
                .token_amount(scenario.plane.hoard_token.address)
                .await,
            SETS + CASH_ATOMS - 1
        );
        let hoard = HoardAccount::decode(&scenario.data(scenario.plane.hoard.address).await)
            .expect("hoard decodes");
        let supply =
            SupplyLedgerAccount::decode(&scenario.data(scenario.plane.supply.address).await)
                .expect("supply decodes");
        let kernel = KernelAccount::decode(&scenario.data(scenario.plane.kernel.address).await)
            .expect("kernel decodes");
        assert_eq!(hoard.collateral_atoms, SETS - 1);
        assert_eq!(supply.internal_supply[0], SETS - lot);
        assert_eq!(supply.external_supply[0], 0);
        assert_eq!(kernel.total_supply[0], SETS - lot);
        assert_eq!(
            scenario.data(scenario.plane.resolution.address).await,
            resolution_before
        );
        assert_eq!(
            scenario.data(scenario.plane.position.address).await,
            position_before,
            "bearer authority is positionless"
        );
        println!("native d{degree} external exact lot {lot}: {units} CU");
    }
}

#[tokio::test]
async fn a_late_native_bearer_transfer_failure_rolls_back_the_prior_burn() {
    let lot = exact_lot(3);
    let mut scenario = Scenario::start_external_resolved(3, lot, u64::MAX).await;
    let watched = [
        scenario.plane.hoard.address,
        scenario.plane.kernel.address,
        scenario.plane.supply.address,
        scenario.plane.resolution.address,
        scenario.plane.hoard_token.address,
        ACTOR_TOKEN,
        OUTCOME_SOURCE,
        scenario.plane.outcome_mints[0].address,
    ];
    let before = snapshot(&mut scenario, &watched).await;
    assert!(scenario
        .try_send(scenario.external_exit(lot))
        .await
        .0
        .is_err());
    assert_eq!(snapshot(&mut scenario, &watched).await, before);
}

#[tokio::test]
async fn native_bearer_exit_rejects_hostile_roles_modes_windows_and_mint_vectors() {
    let lot = exact_lot(2);
    let mut scenario = Scenario::start_external(2, lot, 0).await;
    let unresolved_watched = [
        scenario.plane.hoard.address,
        scenario.plane.kernel.address,
        scenario.plane.supply.address,
        OUTCOME_SOURCE,
        scenario.plane.outcome_mints[0].address,
    ];
    let unresolved_before = snapshot(&mut scenario, &unresolved_watched).await;
    assert!(scenario
        .try_send(scenario.external_exit(lot))
        .await
        .0
        .is_err());
    assert_eq!(
        snapshot(&mut scenario, &unresolved_watched).await,
        unresolved_before
    );

    let mut scenario = Scenario::start_external_resolved(2, lot, 0).await;
    let resolved_before = snapshot(&mut scenario, &unresolved_watched).await;
    let mut writable_resolution = scenario.external_exit(lot);
    writable_resolution.accounts[6].is_writable = true;
    assert!(scenario.try_send(writable_resolution).await.0.is_err());
    let mut aliased_source = scenario.external_exit(lot);
    aliased_source.accounts[14] = aliased_source.accounts[11].clone();
    assert!(scenario.try_send(aliased_source).await.0.is_err());
    let mut incomplete_mints = scenario.external_exit(lot);
    incomplete_mints.accounts.pop();
    assert!(scenario.try_send(incomplete_mints).await.0.is_err());
    assert_eq!(
        snapshot(&mut scenario, &unresolved_watched).await,
        resolved_before
    );

    let actor = actor_keypair();
    let plane = smooth_plane_with_external(actor.pubkey(), 2, 4, 4, lot);
    let terms = TermsAccount::decode(
        &plane
            .accounts
            .iter()
            .find(|account| account.address == plane.terms.address)
            .expect("terms account")
            .data,
    )
    .expect("terms decode");
    let preset = NativeResolutionAccount {
        market: plane.market_id,
        terms: terms.terms,
        feed: terms.feed,
        window: Hash32::from_bytes([0xa5; 32]),
        feed_cursor: 104,
        sealed_end_bucket_exclusive: 103,
        repair_generation: 0,
        resolved_slot: 0,
        mode: RESOLUTION_MODE_PRESET,
        payout_index: 0,
        outcome_count: 0,
        resolved_value: 0,
        vector: PayoutVectorBytes::ZERO,
        stored_bump: plane.resolution.bump,
        flags: 0,
    };
    let preset_bytes = encode(NATIVE_RESOLUTION_LEN, |out| preset.encode(out));
    let mut wrong_mode =
        Scenario::start_custom(force_resolved_plane(plane, preset_bytes), lot, 0).await;
    assert!(wrong_mode
        .try_send(wrong_mode.external_exit(lot))
        .await
        .0
        .is_err());

    let actor = actor_keypair();
    let plane = smooth_plane_with_external(actor.pubkey(), 2, 4, 4, lot);
    let terms = TermsAccount::decode(
        &plane
            .accounts
            .iter()
            .find(|account| account.address == plane.terms.address)
            .expect("terms account")
            .data,
    )
    .expect("terms decode");
    let derived = NativeResolutionAccount {
        market: plane.market_id,
        terms: terms.terms,
        feed: terms.feed,
        window: Hash32::from_bytes([0xa6; 32]),
        feed_cursor: 104,
        sealed_end_bucket_exclusive: 103,
        repair_generation: 0,
        resolved_slot: 0,
        mode: RESOLUTION_MODE_DERIVED_POINT,
        payout_index: PAYOUT_INDEX_UNRESOLVED,
        outcome_count: OUTCOMES,
        resolved_value: 4,
        vector: PayoutVectorBytes {
            denominator: DENOMINATOR,
            weights: expected_weights(2),
        },
        stored_bump: plane.resolution.bump,
        flags: 0,
    };
    let mut zero_window = encode(NATIVE_RESOLUTION_LEN, |out| derived.encode(out));
    zero_window[98..130].fill(0);
    let mut wrong_window =
        Scenario::start_custom(force_resolved_plane(plane, zero_window), lot, 0).await;
    assert!(wrong_window
        .try_send(wrong_window.external_exit(lot))
        .await
        .0
        .is_err());
}

#[tokio::test]
async fn occupation_degrees_one_through_three_persist_retry_and_redeem_v4() {
    for degree in 1..=3 {
        let mut scenario = Scenario::start_occupation(degree, 4, 4, 0, 0, None).await;
        assert_eq!(
            scenario.resolve_occupation().accounts.len(),
            observe_resolve::OCCUPATION_RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES)
        );
        let resolution_account = scenario.account(scenario.plane.resolution.address).await;
        assert_eq!(resolution_account.data.len(), OCCUPATION_RESOLUTION_LEN);
        assert_eq!(
            resolution_account.lamports,
            Rent::default()
                .minimum_balance(OCCUPATION_RESOLUTION_LEN)
                .max(1)
        );

        let (result, resolve_units) = scenario.try_send(scenario.resolve_occupation()).await;
        result.expect("occupation resolve succeeds");
        let record_bytes = scenario.data(scenario.plane.resolution.address).await;
        let record = OccupationResolutionAccount::decode(&record_bytes)
            .expect("v4 occupation record decodes");
        assert_eq!(record.mode, RESOLUTION_MODE_DERIVED_QUANTIZED_OCCUPATION);
        assert_eq!(record.resolved_value, 0);
        assert_eq!(record.outcome_count, OUTCOMES);
        assert_eq!(record.vector.denominator, DENOMINATOR);
        assert_eq!(record.vector.weights, expected_weights(degree));
        assert_eq!(
            record.archive_commitment,
            source_archive_commitment(&scenario.plane)
        );
        assert_eq!(record.statistic, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
        assert_eq!(record.finalization, OCCUPATION_FINALIZATION_EXACT_ONLY);
        assert_eq!(
            (record.sample_count, record.coverage_count, record.gap_count),
            (3, 3, 0)
        );

        let retry_watched = [
            scenario.plane.market.address,
            scenario.plane.kernel.address,
            scenario.plane.supply.address,
            scenario.plane.resolution.address,
        ];
        let before_retry = snapshot(&mut scenario, &retry_watched).await;
        let (retry, retry_units) = scenario.try_send(scenario.resolve_occupation()).await;
        retry.expect("exact occupation retry is idempotent");
        assert_eq!(snapshot(&mut scenario, &retry_watched).await, before_retry);

        let lot = exact_lot(degree);
        let watched = [
            scenario.plane.position.address,
            scenario.plane.hoard.address,
            scenario.plane.kernel.address,
            scenario.plane.supply.address,
            scenario.plane.replay.address,
            scenario.plane.resolution.address,
        ];
        let before_sub_lot = snapshot(&mut scenario, &watched).await;
        assert!(scenario
            .try_send(scenario.redeem(0, 0, lot - 1))
            .await
            .0
            .is_err());
        assert_eq!(snapshot(&mut scenario, &watched).await, before_sub_lot);

        let (redeemed, redeem_units) = scenario.try_send(scenario.redeem(0, 0, lot)).await;
        redeemed.expect("occupation vector redeems an exact internal lot");
        let position =
            PositionAccount::decode(&scenario.data(scenario.plane.position.address).await)
                .expect("position decodes");
        assert_eq!(position.internal[0], SETS - lot);
        assert_eq!(position.cash_atoms, CASH_ATOMS + 1);
        assert_eq!(
            scenario.data(scenario.plane.resolution.address).await,
            record_bytes
        );
        println!(
            "occupation-v4 d{degree}: accounts={} resolution_bytes={} rent={} resolve={resolve_units} CU retry={retry_units} CU internal={redeem_units} CU",
            observe_resolve::OCCUPATION_RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES),
            OCCUPATION_RESOLUTION_LEN,
            Rent::default().minimum_balance(OCCUPATION_RESOLUTION_LEN),
        );
    }
}

#[tokio::test]
async fn occupation_span_one_two_initial_resolve_cu_profile() {
    // `units * 5 / 4 <= 1_400_000` is the chosen 25% operating-headroom gate.
    const MAX_ADMISSIBLE_CU: u64 = 1_120_000;
    let mut admissible = Vec::new();
    for span in 1..=2_u64 {
        for degree in 1..=3_u8 {
            let actor = actor_keypair();
            let plane = occupation_plane_with_span(actor.pubkey(), degree, span);
            let mut scenario = Scenario::start_plane(actor, plane, 0, 0, None).await;
            let (result, units) = scenario.try_send(scenario.resolve_occupation()).await;
            result.expect("bounded occupation profile resolves");
            let record = OccupationResolutionAccount::decode(
                &scenario.data(scenario.plane.resolution.address).await,
            )
            .expect("span-profile v4 record");
            assert_eq!(record.sample_count, span);
            assert_eq!(record.coverage_count, span);
            assert_eq!(record.gap_count, 0);
            if units <= MAX_ADMISSIBLE_CU {
                admissible.push((span, degree, units));
            }
            println!("occupation-v4 span={span} d{degree}: initial={units} CU");
        }
    }
    assert!(
        admissible.is_empty(),
        "review occupation admission: profile unexpectedly changed to {admissible:?}"
    );
}

#[tokio::test]
async fn occupation_statistic_seven_routes_only_its_named_finalizer() {
    let actor = actor_keypair();
    let plane = occupation_plane_with_statistic(
        actor.pubkey(),
        2,
        4,
        4,
        0,
        STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07,
    );
    let mut scenario = Scenario::start_custom(plane, 0, 0).await;
    let (result, units) = scenario.try_send(scenario.resolve_occupation()).await;
    result.expect("statistic seven occupation resolves through v4");
    let record = OccupationResolutionAccount::decode(
        &scenario.data(scenario.plane.resolution.address).await,
    )
    .expect("v4 statistic-seven record");
    assert_eq!(
        record.statistic,
        STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07
    );
    assert_eq!(
        record.finalization,
        OCCUPATION_FINALIZATION_LARGEST_REMAINDER_V1
    );
    assert_eq!(record.vector.weights, expected_weights(2));
    println!("occupation-v4 statistic-7 named finalizer: {units} CU");
}

#[tokio::test]
async fn occupation_bearer_exit_uses_live_v4_and_exact_lots() {
    for degree in 1..=3 {
        let lot = exact_lot(degree);
        let mut scenario = Scenario::start_occupation(degree, 4, 4, lot, 0, None).await;
        scenario
            .try_send(scenario.resolve_occupation())
            .await
            .0
            .expect("occupation resolves before bearer exit");
        let watched = [
            scenario.plane.hoard.address,
            scenario.plane.kernel.address,
            scenario.plane.supply.address,
            scenario.plane.resolution.address,
            scenario.plane.position.address,
            scenario.plane.hoard_token.address,
            ACTOR_TOKEN,
            OUTCOME_SOURCE,
            scenario.plane.outcome_mints[0].address,
        ];
        let before_sub_lot = snapshot(&mut scenario, &watched).await;
        assert!(scenario
            .try_send(scenario.external_exit(lot - 1))
            .await
            .0
            .is_err());
        assert_eq!(snapshot(&mut scenario, &watched).await, before_sub_lot);

        let record_before = scenario.data(scenario.plane.resolution.address).await;
        let position_before = scenario.data(scenario.plane.position.address).await;
        let (result, units) = scenario.try_send(scenario.external_exit(lot)).await;
        result.expect("occupation bearer exit consumes an exact lot");
        assert_eq!(scenario.token_amount(OUTCOME_SOURCE).await, 0);
        assert_eq!(scenario.token_amount(ACTOR_TOKEN).await, 1);
        assert_eq!(
            scenario.data(scenario.plane.resolution.address).await,
            record_before
        );
        assert_eq!(
            scenario.data(scenario.plane.position.address).await,
            position_before,
            "bearer exit remains positionless"
        );
        println!("occupation-v4 d{degree} external exact lot {lot}: {units} CU");
    }
}

#[tokio::test]
async fn occupation_midpoints_gaps_substitution_modes_and_conflicts_refuse_atomically() {
    for degree in 1..=3 {
        let mut midpoint = Scenario::start_occupation(degree, 3, 5, 0, 0, None).await;
        let watched = [
            midpoint.plane.market.address,
            midpoint.plane.kernel.address,
            midpoint.plane.supply.address,
            midpoint.plane.resolution.address,
        ];
        let before = snapshot(&mut midpoint, &watched).await;
        assert!(midpoint
            .try_send(midpoint.resolve_occupation())
            .await
            .0
            .is_err());
        assert_eq!(snapshot(&mut midpoint, &watched).await, before);
    }

    let mut substituted = Scenario::start_occupation(2, 4, 4, 0, 0, None).await;
    let watched = [
        substituted.plane.market.address,
        substituted.plane.kernel.address,
        substituted.plane.supply.address,
        substituted.plane.resolution.address,
    ];
    let before = snapshot(&mut substituted, &watched).await;
    let mut wrong_archive = substituted.resolve_occupation();
    wrong_archive.accounts[9] = AccountMeta::new_readonly(SUBSTITUTE_ARCHIVE, false);
    assert!(substituted.try_send(wrong_archive).await.0.is_err());
    let mut redundant_projection = substituted.resolve_occupation();
    redundant_projection
        .accounts
        .insert(10, AccountMeta::new_readonly(BUFFER_ACCOUNT, false));
    assert!(substituted.try_send(redundant_projection).await.0.is_err());
    assert_eq!(snapshot(&mut substituted, &watched).await, before);

    let actor = actor_keypair();
    let mut missing = occupation_plane_with_external(actor.pubkey(), 2, 4, 4, 0);
    let archive_address = missing.source_archive.address;
    account_mut(&mut missing, archive_address).data[3] = 2;
    let mut missing = Scenario::start_custom(missing, 0, 0).await;
    let watched = [
        missing.plane.market.address,
        missing.plane.kernel.address,
        missing.plane.supply.address,
        missing.plane.resolution.address,
    ];
    let before = snapshot(&mut missing, &watched).await;
    assert!(missing
        .try_send(missing.resolve_occupation())
        .await
        .0
        .is_err());
    assert_eq!(snapshot(&mut missing, &watched).await, before);

    let actor = actor_keypair();
    let mut wrong_len = occupation_plane_with_external(actor.pubkey(), 2, 4, 4, 0);
    let terms = TermsAccount::decode(
        &wrong_len
            .accounts
            .iter()
            .find(|account| account.address == wrong_len.terms.address)
            .expect("terms account")
            .data,
    )
    .unwrap();
    let v3 = NativeResolutionAccount::unresolved(
        wrong_len.market_id,
        terms.terms,
        terms.feed,
        wrong_len.resolution.bump,
    );
    let resolution_address = wrong_len.resolution.address;
    account_mut(&mut wrong_len, resolution_address).data =
        encode(NATIVE_RESOLUTION_LEN, |out| v3.encode(out));
    let mut wrong_len = Scenario::start_custom(wrong_len, 0, 0).await;
    assert!(wrong_len
        .try_send(wrong_len.resolve_occupation())
        .await
        .0
        .is_err());

    let actor = actor_keypair();
    let mut wrong_mode = occupation_plane_with_external(actor.pubkey(), 2, 4, 4, 0);
    let resolution_address = wrong_mode.resolution.address;
    account_mut(&mut wrong_mode, resolution_address).data[162] = 2;
    let mut wrong_mode = Scenario::start_custom(wrong_mode, 0, 0).await;
    assert!(wrong_mode
        .try_send(wrong_mode.resolve_occupation())
        .await
        .0
        .is_err());

    let actor = actor_keypair();
    let plane = occupation_plane_with_external(actor.pubkey(), 2, 4, 4, 0);
    let conflict = occupation_record(&plane, 2, Hash32::from_bytes([0x92; 32]));
    let bytes = encode(OCCUPATION_RESOLUTION_LEN, |out| conflict.encode(out));
    let mut conflict = Scenario::start_custom(force_resolved_plane(plane, bytes), 0, 0).await;
    let resolution_before = conflict.data(conflict.plane.resolution.address).await;
    assert!(conflict
        .try_send(conflict.resolve_occupation())
        .await
        .0
        .is_err());
    assert_eq!(
        conflict.data(conflict.plane.resolution.address).await,
        resolution_before
    );
}

#[tokio::test]
async fn occupation_late_resolve_and_bearer_failures_roll_back() {
    let mut resolve = Scenario::start_occupation(3, 4, 4, 0, 0, Some((0, u64::MAX))).await;
    let watched = [
        resolve.plane.market.address,
        resolve.plane.kernel.address,
        resolve.plane.supply.address,
        resolve.plane.resolution.address,
    ];
    let before = snapshot(&mut resolve, &watched).await;
    assert!(resolve
        .try_send(resolve.resolve_occupation())
        .await
        .0
        .is_err());
    assert_eq!(snapshot(&mut resolve, &watched).await, before);

    let lot = exact_lot(3);
    let mut bearer = Scenario::start_occupation(3, 4, 4, lot, u64::MAX, None).await;
    bearer
        .try_send(bearer.resolve_occupation())
        .await
        .0
        .expect("occupation resolves before late bearer failure");
    let watched = [
        bearer.plane.hoard.address,
        bearer.plane.kernel.address,
        bearer.plane.supply.address,
        bearer.plane.resolution.address,
        bearer.plane.hoard_token.address,
        ACTOR_TOKEN,
        OUTCOME_SOURCE,
        bearer.plane.outcome_mints[0].address,
    ];
    let before = snapshot(&mut bearer, &watched).await;
    assert!(bearer.try_send(bearer.external_exit(lot)).await.0.is_err());
    assert_eq!(snapshot(&mut bearer, &watched).await, before);
}
