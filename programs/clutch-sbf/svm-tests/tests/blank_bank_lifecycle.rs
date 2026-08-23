//! Blank-bank construction through typed artifacts and public PDA constructors.
//!
//! The successful cases install no Clutch-owned account at genesis. A wallet
//! creates a real Token-2022 collateral mint, uploads and seals the canonical
//! policy/grid/Terms artifacts, creates Realm and Profile, and founds a market.
//! Degree zero receives the 165-byte v2 resolution record; point degree one
//! receives the 319-byte native v3 record; occupation degree one receives the
//! distinct 383-byte v4 record. Predictable state and token PDAs are deliberately
//! prefunded before construction to prove that SOL donations cannot squat them.

use {
    clutch_sbf::{
        error::ClutchError, instructions::market_init, loader_state::UPGRADEABLE_LOADER_ID, seeds,
    },
    clutch_solana_layout::{
        account_len,
        artifact::{ArtifactKind, ARTIFACT_CHUNK_BYTES},
        canonical_market_id, canonical_realm_id,
        native_resolution::{NativeResolutionAccount, NATIVE_RESOLUTION_LEN},
        occupation_resolution::{
            OccupationResolutionAccount, OCCUPATION_RESOLUTION_LEN,
            STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
        },
        Hash32, Intent, PriceGridAccount, ProfileAccount, RealmAccount, ResolutionAccount,
        TermsAccount, MAX_GRID_TICKS, MAX_OUTCOMES, PAYOUT_MAP_UNUSED,
    },
    clutch_svm_fixture::{
        compute_unit_limit_data, fixture_policy, fixture_policy_identity, fixture_terms,
        layout_request, COMPUTE_BUDGET, PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM, TOKEN_2022,
    },
    solana_account::{Account, AccountSharedData},
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_pack::Pack,
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_signer::Signer,
    solana_system_interface::instruction as system_instruction,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
    spl_token_2022_interface::{
        instruction as token_instruction,
        instruction::AuthorityType,
        state::{Account as TokenAccount, Mint},
    },
};

const CLOCK_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
const REALM_NONCE: u64 = 0x71;
const MARKET_NONCE: u64 = 0x92;
const OUTCOMES: u8 = 2;

fn derive(seeds: &[&[u8]]) -> (Address, u8) {
    Address::find_program_address(seeds, &PROGRAM_ID)
}

fn token_2022_programdata() -> Address {
    Address::find_program_address(
        &[TOKEN_2022.as_ref()],
        &Address::new_from_array(UPGRADEABLE_LOADER_ID),
    )
    .0
}

fn stage_address(funder: Address, kind: ArtifactKind, context: Hash32, digest: Hash32) -> Address {
    derive(&[
        seeds::SEED_ARTIFACT_STAGE,
        funder.as_ref(),
        &[kind.byte()],
        &context.bytes(),
        &digest.bytes(),
    ])
    .0
}

fn final_address(kind: ArtifactKind, context: Hash32, digest: Hash32) -> (Address, u8) {
    let seed = match kind {
        ArtifactKind::CollateralPolicy => seeds::SEED_POLICY,
        ArtifactKind::PriceGrid => seeds::SEED_GRID,
        ArtifactKind::Terms => seeds::SEED_TERMS,
        ArtifactKind::BatchPolicy => seeds::SEED_BATCH_POLICY,
        ArtifactKind::DirectBatchPolicyV3 => seeds::SEED_DIRECT_BATCH_POLICY_V3,
        kind @ (ArtifactKind::NativeClaimBasisV1
        | ArtifactKind::EvidenceOnlyRecoveryPolicyV1
        | ArtifactKind::ProductTemplateV4
        | ArtifactKind::PriceMeasurePolicyV1
        | ArtifactKind::MarketGenesisProfileV2
        | ArtifactKind::SeriesFundingQuoteV1
        | ArtifactKind::SeriesAttachmentPlanV1
        | ArtifactKind::SeriesPlanV5
        | ArtifactKind::SeriesFundingTermsV2
        | ArtifactKind::RegistryProgramReleaseV1
        | ArtifactKind::RegistryCapabilityProfileV2
        | ArtifactKind::CompiledProductSeriesBundleV1
        | ArtifactKind::SourceReleaseManifestV1
        | ArtifactKind::SourceWorkScheduleV1
        | ArtifactKind::MarketInstancePreimageV2
        | ArtifactKind::SourceReleaseManifestV2
        | ArtifactKind::SeriesFundingQuoteV2
        | ArtifactKind::CompiledProductSeriesBundleV2
        | ArtifactKind::SeriesAttachmentPlanV2
        | ArtifactKind::RegistryCapabilityProfileV3
        | ArtifactKind::SeriesFundingQuoteV3
        | ArtifactKind::CompiledProductSeriesBundleV3
        | ArtifactKind::SeriesAttachmentPlanV3
        | ArtifactKind::SeriesFundingQuoteV4
        | ArtifactKind::CompiledProductSeriesBundleV4
        | ArtifactKind::SeriesAttachmentPlanV4) => {
            return derive(&[
                seeds::SEED_PRODUCT_ARTIFACT_V1,
                &[kind.byte()],
                &digest.bytes(),
            ]);
        }
    };
    derive(&[seed, &context.bytes(), &digest.bytes()])
}

async fn send(
    bank: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> (Result<(), TransactionError>, u64) {
    let blockhash = bank.banks_client.get_latest_blockhash().await.unwrap();
    let mut all = vec![&bank.payer];
    all.extend_from_slice(signers);
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&bank.payer.pubkey()),
        &all,
        blockhash,
    );
    let outcome = bank
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
    bank: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> u64 {
    let (result, units) = send(bank, instructions, signers).await;
    result.expect("transaction should succeed");
    units
}

async fn get(bank: &mut ProgramTestContext, address: Address) -> Option<Account> {
    bank.banks_client.get_account(address).await.unwrap()
}

fn artifact_begin(
    funder: Address,
    stage: Address,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::BeginArtifact {
                kind,
                context,
                digest,
                exact_len: kind.exact_len() as u16,
                expires_slot: 10_000,
            },
        ),
        vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn artifact_write(
    funder: Address,
    stage: Address,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
    cursor: usize,
    body: &[u8],
) -> Instruction {
    let chunk_len = ARTIFACT_CHUNK_BYTES.min(body.len() - cursor);
    let mut chunk = [0; ARTIFACT_CHUNK_BYTES];
    chunk[..chunk_len].copy_from_slice(&body[cursor..cursor + chunk_len]);
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::WriteArtifact {
                kind,
                context,
                digest,
                cursor: cursor as u16,
                chunk_len: chunk_len as u16,
                chunk,
            },
        ),
        vec![
            AccountMeta::new_readonly(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn artifact_seal(
    funder: Address,
    stage: Address,
    final_account: Address,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::SealArtifact {
                kind,
                context,
                digest,
                exact_len: kind.exact_len() as u16,
            },
        ),
        vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new(final_account, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

async fn upload(
    bank: &mut ProgramTestContext,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
    body: &[u8],
) -> Address {
    let funder = bank.payer.pubkey();
    let stage = stage_address(funder, kind, context, digest);
    let final_account = final_address(kind, context, digest).0;
    succeed(
        bank,
        &[
            budget(),
            artifact_begin(funder, stage, kind, context, digest),
        ],
        &[],
    )
    .await;
    let mut cursor = 0;
    while cursor < body.len() {
        succeed(
            bank,
            &[
                budget(),
                artifact_write(funder, stage, kind, context, digest, cursor, body),
            ],
            &[],
        )
        .await;
        cursor += ARTIFACT_CHUNK_BYTES.min(body.len() - cursor);
    }
    succeed(
        bank,
        &[
            budget(),
            artifact_seal(funder, stage, final_account, kind, context, digest),
        ],
        &[],
    )
    .await;
    assert!(get(bank, stage).await.is_none());
    final_account
}

#[derive(Clone)]
struct Founding {
    realm_id: Hash32,
    profile_id: Hash32,
    terms_id: Hash32,
    feed: Hash32,
    policy: Address,
    realm: Address,
    profile: Address,
    grid: Address,
    terms: Address,
    market: Address,
    hoard: Address,
    position: Address,
    kernel: Address,
    replay: Address,
    supply: Address,
    resolution: Address,
    hoard_authority: Address,
    hoard_token: Address,
    outcome_mints: [Address; 2],
    collateral_mint: Address,
    degree: u8,
    occupation: bool,
}

impl Founding {
    fn state_targets(&self) -> [Address; 7] {
        [
            self.market,
            self.hoard,
            self.position,
            self.kernel,
            self.replay,
            self.supply,
            self.resolution,
        ]
    }

    fn create_market(&self, creator: Address) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(creator, true),
            AccountMeta::new_readonly(self.realm, false),
            AccountMeta::new_readonly(self.profile, false),
            AccountMeta::new_readonly(self.terms, false),
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.hoard, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.kernel, false),
            AccountMeta::new(self.replay, false),
            AccountMeta::new(self.supply, false),
            AccountMeta::new(self.resolution, false),
            AccountMeta::new_readonly(self.policy, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.collateral_mint, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(self.hoard_authority, false),
            AccountMeta::new(self.hoard_token, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
        ];
        metas.extend(
            self.outcome_mints
                .iter()
                .map(|mint| AccountMeta::new(*mint, false)),
        );
        assert_eq!(metas.len(), market_init::account_count(OUTCOMES));
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::CreateMarket {
                    realm: self.realm_id,
                    profile: self.profile_id,
                    market_nonce: MARKET_NONCE,
                    outcome_count: OUTCOMES,
                    terms: self.terms_id,
                    feed: self.feed,
                },
            ),
            metas,
        )
    }
}

async fn create_collateral_mint(bank: &mut ProgramTestContext, mint: &Keypair) {
    let rent = bank.banks_client.get_rent().await.unwrap();
    let payer = bank.payer.pubkey();
    let holder = Keypair::new_from_array([0x43; 32]);
    succeed(
        bank,
        &[
            system_instruction::create_account(
                &payer,
                &mint.pubkey(),
                rent.minimum_balance(Mint::LEN),
                Mint::LEN as u64,
                &TOKEN_2022,
            ),
            token_instruction::initialize_mint2(&TOKEN_2022, &mint.pubkey(), &payer, None, 6)
                .unwrap(),
            system_instruction::create_account(
                &payer,
                &holder.pubkey(),
                rent.minimum_balance(TokenAccount::LEN),
                TokenAccount::LEN as u64,
                &TOKEN_2022,
            ),
            token_instruction::initialize_account3(
                &TOKEN_2022,
                &holder.pubkey(),
                &mint.pubkey(),
                &payer,
            )
            .unwrap(),
            token_instruction::mint_to(
                &TOKEN_2022,
                &mint.pubkey(),
                &holder.pubkey(),
                &payer,
                &[],
                1,
            )
            .unwrap(),
            token_instruction::set_authority(
                &TOKEN_2022,
                &mint.pubkey(),
                None,
                AuthorityType::MintTokens,
                &payer,
                &[],
            )
            .unwrap(),
        ],
        &[mint, &holder],
    )
    .await;
}

async fn prepare(
    bank: &mut ProgramTestContext,
    degree: u8,
    occupation_statistic: Option<u16>,
) -> Founding {
    let mint = Keypair::new_from_array([0x42; 32]);
    create_collateral_mint(bank, &mint).await;

    let policy_value = fixture_policy(mint.pubkey().to_bytes());
    let (policy_digest, release_id, profile_id) = fixture_policy_identity(policy_value);
    let policy_body = policy_value.encode().unwrap();
    let policy = upload(
        bank,
        ArtifactKind::CollateralPolicy,
        profile_id,
        policy_digest,
        &policy_body,
    )
    .await;

    let realm_id = canonical_realm_id(profile_id, REALM_NONCE);
    let (realm, _) = derive(&[seeds::SEED_REALM, &realm_id.bytes()]);
    let payer = bank.payer.pubkey();
    succeed(
        bank,
        &[
            budget(),
            Instruction::new_with_bytes(
                PROGRAM_ID,
                &layout_request(
                    0,
                    Intent::InitRealm {
                        profile: profile_id,
                        realm_nonce: REALM_NONCE,
                        max_outcomes: MAX_OUTCOMES as u8,
                        profile_version: 2,
                    },
                ),
                vec![
                    AccountMeta::new(payer, true),
                    AccountMeta::new(realm, false),
                    AccountMeta::new_readonly(policy, false),
                    AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                    AccountMeta::new_readonly(RENT_SYSVAR, false),
                ],
            ),
        ],
        &[],
    )
    .await;

    let (profile, _) = derive(&[seeds::SEED_PROFILE, &realm_id.bytes(), &profile_id.bytes()]);
    succeed(
        bank,
        &[
            budget(),
            Instruction::new_with_bytes(
                PROGRAM_ID,
                &layout_request(
                    0,
                    Intent::InitProfileV2 {
                        realm: realm_id,
                        collateral_policy_id: policy_digest,
                        adapter_release_id: release_id,
                        profile_version: 2,
                    },
                ),
                vec![
                    AccountMeta::new(payer, true),
                    AccountMeta::new(profile, false),
                    AccountMeta::new_readonly(realm, false),
                    AccountMeta::new_readonly(policy, false),
                    AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                    AccountMeta::new_readonly(RENT_SYSVAR, false),
                    AccountMeta::new_readonly(TOKEN_2022, false),
                    AccountMeta::new_readonly(token_2022_programdata(), false),
                ],
            ),
        ],
        &[],
    )
    .await;

    let mut ticks = [0; MAX_GRID_TICKS];
    ticks[..3].copy_from_slice(&[1, 2, 3]);
    let mut grid_value = PriceGridAccount {
        grid: Hash32::ZERO,
        realm: realm_id,
        price_scale: 1_000_000,
        tick_count: 3,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid_value.grid = grid_value.recomputed_grid_id().unwrap();
    let (_, grid_bump) = final_address(ArtifactKind::PriceGrid, realm_id, grid_value.grid);
    grid_value.stored_bump = grid_bump;
    let mut grid_body = vec![0; account_len::PRICE_GRID];
    grid_value.encode(&mut grid_body).unwrap();
    let grid = upload(
        bank,
        ArtifactKind::PriceGrid,
        realm_id,
        grid_value.grid,
        &grid_body,
    )
    .await;

    let feed = Hash32::from_bytes([0x61; 32]);
    let mut terms_value = fixture_terms(realm_id, profile_id, feed);
    terms_value.price_grid = grid_value.grid;
    terms_value.basis_degree = degree;
    if degree == 1 {
        terms_value.knot_count = 2;
        terms_value.knots = [0; clutch_solana_layout::MAX_KNOTS];
        terms_value.knots[0] = 10;
        terms_value.knots[1] = 20;
        terms_value.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    }
    if let Some(statistic) = occupation_statistic {
        terms_value.statistic_id = statistic;
    }
    terms_value.terms = Hash32::ZERO;
    terms_value.terms = terms_value.recomputed_terms_digest().unwrap();
    let terms_id = terms_value.terms;
    let (_, terms_bump) = final_address(ArtifactKind::Terms, realm_id, terms_id);
    terms_value.stored_bump = terms_bump;
    let mut terms_body = vec![0; account_len::TERMS];
    terms_value.encode(&mut terms_body).unwrap();
    let terms = upload(bank, ArtifactKind::Terms, realm_id, terms_id, &terms_body).await;

    let market_id = canonical_market_id(realm_id, profile_id, MARKET_NONCE);
    let owner = payer.to_bytes();
    let (market, _) = derive(&[seeds::SEED_MARKET, &realm_id.bytes(), &market_id.bytes()]);
    let (hoard, _) = derive(&[seeds::SEED_HOARD, &market_id.bytes()]);
    let (position, _) = derive(&[seeds::SEED_POSITION, &market_id.bytes(), &owner]);
    let (kernel, _) = derive(&[seeds::SEED_KERNEL, &market_id.bytes()]);
    let generation = 0_u64.to_le_bytes();
    let (replay, _) = derive(&[seeds::SEED_REPLAY, &market_id.bytes(), &owner, &generation]);
    let (supply, _) = derive(&[seeds::SEED_SUPPLY, &market_id.bytes()]);
    let (resolution, _) = derive(&[seeds::SEED_RESOLUTION, &market_id.bytes()]);
    let (hoard_authority, _) = derive(&[seeds::SEED_HOARD_AUTHORITY, &market_id.bytes()]);
    let (hoard_token, _) = derive(&[seeds::SEED_HOARD_TOKEN, &market_id.bytes()]);
    let outcome_mints =
        [0_u8, 1].map(|index| derive(&[seeds::SEED_OUTCOME_MINT, &market_id.bytes(), &[index]]).0);

    Founding {
        realm_id,
        profile_id,
        terms_id,
        feed,
        policy,
        realm,
        profile,
        grid,
        terms,
        market,
        hoard,
        position,
        kernel,
        replay,
        supply,
        resolution,
        hoard_authority,
        hoard_token,
        outcome_mints,
        collateral_mint: mint.pubkey(),
        degree,
        occupation: occupation_statistic.is_some(),
    }
}

fn budget() -> Instruction {
    Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(1_400_000), vec![])
}

async fn assert_sealed_prerequisites(bank: &mut ProgramTestContext, f: &Founding) {
    assert_eq!(get(bank, f.policy).await.unwrap().owner, PROGRAM_ID);
    assert!(RealmAccount::decode(&get(bank, f.realm).await.unwrap().data).is_ok());
    assert!(ProfileAccount::decode(&get(bank, f.profile).await.unwrap().data).is_ok());
    assert!(PriceGridAccount::decode(&get(bank, f.grid).await.unwrap().data).is_ok());
    assert!(TermsAccount::decode(&get(bank, f.terms).await.unwrap().data).is_ok());
}

async fn prefund_and_found(degree: u8, occupation_statistic: Option<u16>) {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    let mut bank = test.start_with_context().await;
    let founding = prepare(&mut bank, degree, occupation_statistic).await;
    assert_sealed_prerequisites(&mut bank, &founding).await;
    for target in founding
        .state_targets()
        .into_iter()
        .chain(founding.outcome_mints)
        .chain([founding.hoard_token])
    {
        assert!(get(&mut bank, target).await.is_none());
    }

    let rent = bank.banks_client.get_rent().await.unwrap();
    let resolution_len = if degree == 0 {
        account_len::RESOLUTION
    } else if occupation_statistic.is_some() {
        OCCUPATION_RESOLUTION_LEN
    } else {
        NATIVE_RESOLUTION_LEN
    };
    let resolution_excess = rent.minimum_balance(resolution_len) + 7_777;
    let first_mint_prefund = rent.minimum_balance(Mint::LEN) + 6_666;
    let hoard_excess = rent.minimum_balance(170) + 8_888;
    let payer = bank.payer.pubkey();
    succeed(
        &mut bank,
        &[
            system_instruction::transfer(&payer, &founding.resolution, resolution_excess),
            system_instruction::transfer(&payer, &founding.outcome_mints[0], first_mint_prefund),
            system_instruction::transfer(&payer, &founding.hoard_token, hoard_excess),
        ],
        &[],
    )
    .await;
    /* The runtime does not permit a public transaction to leave a newly
     * credited zero-data account below its rent-exempt minimum. Injecting the
     * otherwise-valid System account is therefore the only way to exercise
     * the one-lamport recovery branch; the three over-rent targets above were
     * created by ordinary bank transactions and prove the public squatting
     * scenario. */
    bank.set_account(
        &founding.market,
        &AccountSharedData::from(Account {
            lamports: 1,
            data: vec![],
            owner: SYSTEM_PROGRAM,
            executable: false,
            rent_epoch: 0,
        }),
    );

    let units = succeed(&mut bank, &[budget(), founding.create_market(payer)], &[]).await;

    let market = get(&mut bank, founding.market).await.unwrap();
    assert_eq!(market.owner, PROGRAM_ID);
    assert_eq!(market.lamports, rent.minimum_balance(account_len::MARKET));
    let resolution = get(&mut bank, founding.resolution).await.unwrap();
    assert_eq!(resolution.owner, PROGRAM_ID);
    assert_eq!(resolution.lamports, resolution_excess);
    assert_eq!(resolution.data.len(), resolution_len);
    if degree == 0 {
        let record = ResolutionAccount::decode(&resolution.data).unwrap();
        assert!(!record.is_resolved());
    } else if occupation_statistic.is_some() {
        let record = OccupationResolutionAccount::decode(&resolution.data).unwrap();
        assert!(!record.is_resolved());
    } else {
        let record = NativeResolutionAccount::decode(&resolution.data).unwrap();
        assert!(!record.is_resolved());
    }
    let first_mint = get(&mut bank, founding.outcome_mints[0]).await.unwrap();
    assert_eq!(first_mint.owner, TOKEN_2022);
    assert_eq!(first_mint.lamports, first_mint_prefund);
    let hoard = get(&mut bank, founding.hoard_token).await.unwrap();
    assert_eq!(hoard.owner, TOKEN_2022);
    assert_eq!(hoard.lamports, hoard_excess);
    println!(
        "blank-bank degree={} occupation={} resolution_bytes={} rent={} prefund-safe create_market={} CU",
        founding.degree,
        founding.occupation,
        resolution_len,
        rent.minimum_balance(resolution_len),
        units
    );
}

#[tokio::test]
async fn categorical_and_native_markets_construct_from_only_sealed_artifacts() {
    prefund_and_found(0, None).await;
    prefund_and_found(1, None).await;
    prefund_and_found(1, Some(STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06)).await;
}

#[tokio::test]
async fn a_late_token_target_refusal_rolls_every_earlier_creation_back() {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    let mut bank = test.start_with_context().await;
    let founding = prepare(&mut bank, 1, None).await;
    let blocked = founding.outcome_mints[1];
    bank.set_account(
        &blocked,
        &AccountSharedData::from(Account {
            lamports: 123,
            data: vec![1],
            owner: SYSTEM_PROGRAM,
            executable: false,
            rent_epoch: 0,
        }),
    );

    let payer = bank.payer.pubkey();
    let (result, _) = send(&mut bank, &[budget(), founding.create_market(payer)], &[]).await;
    assert_eq!(
        result,
        Err(TransactionError::InstructionError(
            1,
            InstructionError::Custom(ClutchError::AlreadyInitialized as u32)
        ))
    );
    for target in founding.state_targets() {
        assert!(get(&mut bank, target).await.is_none(), "{target}");
    }
    assert!(get(&mut bank, founding.outcome_mints[0]).await.is_none());
    assert_eq!(get(&mut bank, blocked).await.unwrap().data, vec![1]);
    assert!(get(&mut bank, founding.hoard_token).await.is_none());
}
