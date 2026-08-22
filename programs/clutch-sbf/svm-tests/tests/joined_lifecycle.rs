//! STOP-#1 successor gate: one public blank-bank joined lifecycle per smooth
//! degree (one, two, three), as SBF-executed local-bank evidence.
//!
//! Each degree is ONE continuous walk starting from a bank that holds **no
//! Clutch-owned account at genesis**. The same walk runs under both ELF
//! profiles, mirroring `scripts/run_bringup.sh`'s campaign split:
//!
//! * Under the **default empty-registry ELF** the public prefix executes —
//!   real Token-2022 collateral mint, sealed policy/grid/Terms artifacts
//!   (degree-d smooth Terms), Realm, Profile, and `CreateMarket` allocating
//!   the 319-byte native v3 resolution record — and the walk then *asserts*
//!   the value boundary: the public `InitSourceSpec` route refuses with
//!   `Custom(0x0079)` (`SourceReleaseUnavailable`), `Endow` with no SourceSpec
//!   refuses with `WrongProgramOwner` at the state-role gate, and `Endow` with a canonical
//!   injected SourceSpec still refuses with exactly `Custom(0x0079)` while
//!   leaving every watched account byte-identical. These refusals are asserted
//!   steps of the walk, not obstacles.
//!
//! * Under the explicitly labelled **NON-PRODUCTION mock-source ELF** the
//!   same public prefix continues through the funded segment: the SourceSpec,
//!   Feed, and SourceArchive are created, appended, and sealed through the
//!   PUBLIC `InitSourceSpec`/`InitSourceArchive`/`AppendSourceArchive`/
//!   `SealSourceArchive` route (not genesis injection), then `Endow`, `Split`,
//!   `Materialize`, an ordinary bearer Token-2022 transfer, the source-joined
//!   native point `Resolve`, exact internal redemption of every outcome,
//!   exact-lot bearer redemption, and `WithdrawCash` to zero, with terminal
//!   balances asserted.
//!
//! ## Honest injected-prerequisite inventory (per degree)
//!
//! Default empty-registry campaign — **1 injected prerequisite**:
//!   1. the canonical SourceSpec account image (host-encoded through the same
//!      `initialize_source_spec_account` codec the program uses), injected
//!      mid-test only because the public `InitSourceSpec` route itself refuses
//!      `0x0079` on this ELF — that refusal is asserted first, so the
//!      injection exists solely to sharpen the Endow refusal onto the registry
//!      gate rather than the spec-authentication gate.
//!
//! NON-PRODUCTION mock-source campaign — **4 injected prerequisites**:
//!   1. the mock provider program account (`0xb2..`, executable, owned by
//!      `0xb3..`, body `MOCK-PROVIDER-V1`);
//!   2. the mock deployment evidence account (`0xd4..`, owned by `0xd5..`,
//!      body `DEP1` + generation 19);
//!   3. the mock provider source record account (`0xc3..`, owned by the mock
//!      provider program), whose 77-byte record is additionally rewritten
//!      three times by the host between appends, standing in for the provider
//!      program's own writes — the first three are the NON-Clutch laboratory
//!      provider that the compiled mock registry names, and no Clutch
//!      instruction can (correctly) create or write another program's
//!      accounts; and
//!   4. the program-owned resolution evidence buffer presented to `Resolve`
//!      (no public instruction constructs an arbitrary program-owned buffer;
//!      its bytes are a redundant projection the program checks against the
//!      publicly sealed archive byte-for-byte).
//!
//! Both campaigns fund the two wallets with ordinary genesis lamports (the
//! local equivalent of an airdrop). Everything else — collateral mint and
//! token accounts, policy/grid/Terms artifacts, Realm, Profile, SourceSpec,
//! Feed, SourceArchive and its appends/seal (mock ELF), market plane,
//! and the entire value segment — is created by public wallet transactions.
//!
//! Claim vocabulary: this is SBF-EXECUTED focused local-bank evidence for the
//! sealed instruction set, nothing more. It is not devnet, mainnet, release,
//! or production-source evidence; the mock registry is a laboratory parser.

use {
    clutch_kernel::{BasisMode, MAX_OUTCOMES as KERNEL_MAX_OUTCOMES},
    clutch_sbf::{
        error::ClutchError,
        instructions::{
            cash_exit, external_exit, genesis, market_init, observe_resolve,
            observe_resolve::{BUFFER_VERSION, EVIDENCE_BUFFER_HEADER_BYTES, EVIDENCE_BUFFER_TAG},
            source_ingest, split as seam,
        },
        seeds,
        source::{
            SourceSpecFieldsV1, SourceSpecV1, ORIENTATION_QUOTE_PER_BASE,
            SELECTION_FINALIZED_BUCKET_RECORD,
        },
        source_archive::{
            canonical_window_id, initialize_source_spec_account, CoveragePolicy, FeedIdentity,
            Grid, WindowDomain, SOURCE_SPEC_ACCOUNT_V1_BYTES,
        },
    },
    clutch_solana_layout::{
        account_len,
        artifact::{ArtifactKind, ARTIFACT_CHUNK_BYTES},
        canonical_market_id, canonical_realm_id,
        collateral::ParentProfile,
        native_resolution::{
            NativeResolutionAccount, NATIVE_RESOLUTION_LEN, RESOLUTION_MODE_DERIVED_POINT,
        },
        FeedAccount, Hash32, HoardAccount, Intent, MarketAccount, PayoutVectorBytes,
        PositionAccount, PriceGridAccount, SupplyLedgerAccount, TermsAccount, MAX_GRID_TICKS,
        MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_INDEX_UNRESOLVED, PAYOUT_MAP_UNUSED,
    },
    clutch_solana_reference::{KernelAccount, ReplayAccount, KERNEL_ACCOUNT_LEN},
    clutch_svm_fixture::{
        compute_unit_limit_data, fixture_policy, fixture_terms, layout_request, COMPUTE_BUDGET,
        PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM, TOKEN_2022,
    },
    solana_account::{Account, AccountSharedData},
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_pack::Pack,
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_rent::Rent,
    solana_signer::Signer,
    solana_system_interface::instruction as system_instruction,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
    spl_token_2022_interface::{
        extension::StateWithExtensions,
        instruction as token_instruction,
        instruction::AuthorityType,
        state::{Account as TokenAccount, Mint},
    },
};

const CLOCK_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
const REALM_NONCE: u64 = 7;
const MARKET_NONCE: u64 = 0x92;
const OUTCOMES: u8 = 4;
const SETS: u64 = 64;
const DENOMINATOR: u64 = 64;

/// The laboratory provider identities the compiled mock registry names.
/// These are NOT Clutch accounts; they stand in for an external provider
/// program's deployment, and they are the mock campaign's injected
/// prerequisites 1-3.
const MOCK_ADAPTER: [u8; 32] = [0xa1; 32];
const MOCK_PROGRAM: Address = Address::new_from_array([0xb2; 32]);
const MOCK_PROGRAM_OWNER: Address = Address::new_from_array([0xb3; 32]);
const MOCK_DEPLOYMENT: Address = Address::new_from_array([0xd4; 32]);
const MOCK_DEPLOYMENT_OWNER: Address = Address::new_from_array([0xd5; 32]);
const MOCK_SOURCE: Address = Address::new_from_array([0xc3; 32]);
const DEPLOYMENT_GENERATION: u64 = 19;

/// Injected prerequisite 4 of the mock campaign: the caller-supplied evidence
/// buffer `Resolve` reads. Deliberately not derived — its bytes are the claim,
/// not the state — but the program requires it program-owned, and no public
/// instruction writes an arbitrary program-owned buffer.
const BUFFER_ACCOUNT: Address = Address::new_from_array([0x9c; 32]);

fn actor_keypair() -> Keypair {
    Keypair::new_from_array([0x51; 32])
}

fn bearer_keypair() -> Keypair {
    Keypair::new_from_array([0x52; 32])
}

fn collateral_mint_keypair() -> Keypair {
    Keypair::new_from_array([0x53; 32])
}

fn actor_collateral_keypair() -> Keypair {
    Keypair::new_from_array([0x54; 32])
}

fn bearer_collateral_keypair() -> Keypair {
    Keypair::new_from_array([0x55; 32])
}

fn actor_outcome_keypair() -> Keypair {
    Keypair::new_from_array([0x56; 32])
}

fn bearer_outcome_keypair() -> Keypair {
    Keypair::new_from_array([0x57; 32])
}

fn derive(parts: &[&[u8]]) -> (Address, u8) {
    Address::find_program_address(parts, &PROGRAM_ID)
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

fn budget() -> Instruction {
    Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(1_400_000), vec![])
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
    result.expect("transaction succeeds");
    units
}

async fn refuse(
    bank: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
    code: ClutchError,
) {
    let (result, _) = send(bank, instructions, signers).await;
    assert_eq!(
        result,
        Err(TransactionError::InstructionError(
            1,
            InstructionError::Custom(code as u32)
        ))
    );
}

async fn get(bank: &mut ProgramTestContext, address: Address) -> Option<Account> {
    bank.banks_client.get_account(address).await.unwrap()
}

async fn existing(bank: &mut ProgramTestContext, address: Address) -> Account {
    get(bank, address).await.expect("account exists")
}

async fn snapshot(bank: &mut ProgramTestContext, addresses: &[Address]) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(addresses.len());
    for address in addresses {
        out.push(existing(bank, *address).await.data);
    }
    out
}

fn clock(account: &Account) -> (u64, u64) {
    let slot = u64::from_le_bytes(account.data[0..8].try_into().unwrap());
    let unix = i64::from_le_bytes(account.data[32..40].try_into().unwrap());
    (
        slot,
        u64::try_from(unix).expect("ProgramTest clock is non-negative"),
    )
}

fn injected(owner: Address, data: Vec<u8>, executable: bool) -> AccountSharedData {
    AccountSharedData::from(Account {
        lamports: Rent::default().minimum_balance(data.len()).max(1),
        data,
        owner,
        executable,
        rent_epoch: 0,
    })
}

/* ------------------------------------------------------------------------ */
/* Artifact route                                                            */
/* ------------------------------------------------------------------------ */

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
    let prefix = match kind {
        ArtifactKind::CollateralPolicy => seeds::SEED_POLICY,
        ArtifactKind::PriceGrid => seeds::SEED_GRID,
        ArtifactKind::Terms => seeds::SEED_TERMS,
        ArtifactKind::BatchPolicy => seeds::SEED_BATCH_POLICY,
        ArtifactKind::DirectBatchPolicyV3 => seeds::SEED_DIRECT_BATCH_POLICY_V3,
    };
    derive(&[prefix, &context.bytes(), &digest.bytes()])
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
    let mut chunk = [0_u8; ARTIFACT_CHUNK_BYTES];
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

/* ------------------------------------------------------------------------ */
/* Public mock-source route                                                  */
/* ------------------------------------------------------------------------ */

/// The exact SourceSpec the compiled NON-PRODUCTION mock registry admits.
/// Every field outside the registry pin is chosen by the walk: value atoms
/// are unscaled (`normalized_decimals: 0`) so the archived interval is the
/// spline-domain point itself.
fn walk_spec() -> SourceSpecV1 {
    SourceSpecV1::new(SourceSpecFieldsV1 {
        source_adapter_id: Hash32::from_bytes(MOCK_ADAPTER),
        source_adapter_version: 7,
        parser_id: 11,
        parser_version: 3,
        source_program: MOCK_PROGRAM.to_bytes(),
        source_account: MOCK_SOURCE.to_bytes(),
        deployment_generation: DEPLOYMENT_GENERATION,
        base_asset_id: Hash32::from_bytes([0x01; 32]),
        quote_asset_id: Hash32::from_bytes([0x02; 32]),
        orientation: ORIENTATION_QUOTE_PER_BASE,
        normalized_decimals: 0,
        grid_family_id: 5,
        grid_version: 2,
        bucket_seconds: 1,
        max_staleness_slots: 20,
        max_staleness_seconds: 120,
        max_future_seconds: 2,
        max_confidence_atoms: 10_000,
        max_confidence_bps: 200,
        confidence_multiplier: 2,
        selection_rule: SELECTION_FINALIZED_BUCKET_RECORD,
    })
    .expect("walk source spec is canonical")
}

/// One 77-byte mock provider record, exactly the shape the compiled mock
/// parser reads. Confidence is zero so the admitted interval is the exact
/// point `[price, price]`.
fn provider_record(bucket: u64, sequence: u64, slot: u64, time: u64, price: u128) -> Vec<u8> {
    let mut out = vec![0_u8; 77];
    out[..4].copy_from_slice(b"SRC1");
    out[4..12].copy_from_slice(&DEPLOYMENT_GENERATION.to_le_bytes());
    out[12..20].copy_from_slice(&sequence.to_le_bytes());
    out[20..28].copy_from_slice(&slot.to_le_bytes());
    out[28..36].copy_from_slice(&time.to_le_bytes());
    out[36..44].copy_from_slice(&bucket.to_le_bytes());
    out[44..60].copy_from_slice(&price.to_le_bytes());
    out[60..76].copy_from_slice(&0_u128.to_le_bytes());
    out[76] = 1;
    out
}

/// The redundant Resolve projection of the publicly sealed archive.
///
/// The private `0x45` reference tag/version bytes are pinned here against the
/// production decoder, exactly as the fixture crate pins them: any drift makes
/// the real SBF Resolve transaction fail, not falsely pass.
fn evidence_buffer_bytes(
    window_id: Hash32,
    feed: Hash32,
    start: u64,
    end: u64,
    point: u128,
) -> Vec<u8> {
    let mut window = vec![0x45_u8, 1];
    window.extend_from_slice(&MOCK_ADAPTER); // source adapter
    window.extend_from_slice(&feed.bytes()); // feed spec
    window.extend_from_slice(&7_u32.to_le_bytes()); // source version
    window.extend_from_slice(&1_u32.to_le_bytes()); // evaluator version
    window.extend_from_slice(&5_u32.to_le_bytes()); // grid family
    window.extend_from_slice(&2_u16.to_le_bytes()); // grid version
    window.extend_from_slice(&1_u64.to_le_bytes()); // bucket seconds
    window.extend_from_slice(&start.to_le_bytes());
    window.extend_from_slice(&end.to_le_bytes());
    window.extend_from_slice(&(end + 1).to_le_bytes()); // maturity
    window.extend_from_slice(&0_u64.to_le_bytes()); // repair generation
    window.extend_from_slice(&1_u16.to_le_bytes()); // complete-required coverage
    window.extend_from_slice(&0_u64.to_le_bytes()); // coverage parameter
    window.extend_from_slice(&((end - start) as u16).to_le_bytes());
    for bucket in start..end {
        window.push(1); // accepted observation
        window.extend_from_slice(&bucket.to_le_bytes());
        window.extend_from_slice(&point.to_le_bytes());
        window.extend_from_slice(&point.to_le_bytes());
    }
    let mut data = vec![0_u8; EVIDENCE_BUFFER_HEADER_BYTES];
    data[0] = EVIDENCE_BUFFER_TAG;
    data[1] = BUFFER_VERSION;
    data[2..34].copy_from_slice(&window_id.bytes());
    data[34..36].copy_from_slice(&(window.len() as u16).to_le_bytes());
    data.extend_from_slice(&window);
    data
}

fn point_and_weights(degree: u8) -> (u128, [u64; MAX_OUTCOMES], u64) {
    let mut weights = [0_u64; MAX_OUTCOMES];
    match degree {
        1 => {
            weights[..4].copy_from_slice(&[64, 0, 0, 0]);
            (8, weights, 1)
        }
        2 => {
            weights[..4].copy_from_slice(&[16, 40, 8, 0]);
            (4, weights, 4)
        }
        3 => {
            weights[..4].copy_from_slice(&[8, 24, 24, 8]);
            (4, weights, 8)
        }
        _ => panic!("campaign degree"),
    }
}

/* ------------------------------------------------------------------------ */
/* The joined walk plane                                                     */
/* ------------------------------------------------------------------------ */

#[derive(Clone)]
struct Founding {
    realm_id: Hash32,
    profile_id: Hash32,
    terms_id: Hash32,
    feed_id: Hash32,
    policy: Address,
    realm: Address,
    profile: Address,
    terms: Address,
    market_id: Hash32,
    market: Address,
    hoard: Address,
    position: Address,
    kernel: Address,
    replay: Address,
    supply: Address,
    resolution: Address,
    hoard_authority: Address,
    hoard_token: Address,
    outcome_mints: [Address; 4],
    collateral_mint: Address,
    feed: Address,
    source_spec: Address,
    source_archive: Address,
    window_id: Hash32,
    start_bucket: u64,
    end_bucket: u64,
}

impl Founding {
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
                    feed: self.feed_id,
                },
            ),
            metas,
        )
    }

    fn init_spec(&self, payer: Address, spec: SourceSpecV1) -> Instruction {
        let metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(self.source_spec, false),
            AccountMeta::new(self.feed, false),
            AccountMeta::new_readonly(self.terms, false),
            AccountMeta::new_readonly(MOCK_PROGRAM, false),
            AccountMeta::new_readonly(MOCK_DEPLOYMENT, false),
            AccountMeta::new_readonly(MOCK_SOURCE, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(metas.len(), source_ingest::INIT_SOURCE_SPEC_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::InitSourceSpec {
                    terms: self.terms_id,
                    spec_body: spec.encode_canonical(),
                },
            ),
            metas,
        )
    }

    fn init_archive(&self, payer: Address) -> Instruction {
        let metas = vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(self.source_spec, false),
            AccountMeta::new_readonly(self.feed, false),
            AccountMeta::new_readonly(self.terms, false),
            AccountMeta::new(self.source_archive, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(
            metas.len(),
            source_ingest::INIT_SOURCE_ARCHIVE_ACCOUNT_COUNT
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::InitSourceArchive {
                    terms: self.terms_id,
                },
            ),
            metas,
        )
    }

    fn mutate_archive(&self, sequence: u64, seal: bool) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(self.source_spec, false),
            if seal {
                AccountMeta::new(self.feed, false)
            } else {
                AccountMeta::new_readonly(self.feed, false)
            },
            AccountMeta::new_readonly(self.terms, false),
            AccountMeta::new(self.source_archive, false),
            AccountMeta::new_readonly(MOCK_PROGRAM, false),
            AccountMeta::new_readonly(MOCK_DEPLOYMENT, false),
            AccountMeta::new_readonly(MOCK_SOURCE, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ];
        assert_eq!(
            metas.len(),
            source_ingest::MUTATE_SOURCE_ARCHIVE_ACCOUNT_COUNT
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                if seal {
                    Intent::SealSourceArchive {
                        terms: self.terms_id,
                    }
                } else {
                    Intent::AppendSourceArchive {
                        terms: self.terms_id,
                    }
                },
            ),
            metas,
        )
    }

    fn endow(
        &self,
        actor: Address,
        actor_collateral: Address,
        sequence: u64,
        amount: u64,
    ) -> Instruction {
        let metas = vec![
            AccountMeta::new(actor, true),
            AccountMeta::new_readonly(self.market, false),
            AccountMeta::new_readonly(self.hoard, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.replay, false),
            AccountMeta::new_readonly(self.profile, false),
            AccountMeta::new_readonly(self.policy, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.collateral_mint, false),
            AccountMeta::new(actor_collateral, false),
            AccountMeta::new(self.hoard_token, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(self.terms, false),
            AccountMeta::new_readonly(self.source_spec, false),
        ];
        assert_eq!(metas.len(), genesis::ENDOW_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::Endow {
                    market: self.market_id,
                    owner: Hash32::from_bytes(actor.to_bytes()),
                    amount,
                },
            ),
            metas,
        )
    }

    fn state_prefix(&self, actor: Address) -> Vec<AccountMeta> {
        vec![
            AccountMeta::new(actor, true),
            AccountMeta::new_readonly(self.realm, false),
            AccountMeta::new_readonly(self.profile, false),
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.hoard, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.kernel, false),
            AccountMeta::new(self.replay, false),
            AccountMeta::new(self.supply, false),
        ]
    }

    fn split(
        &self,
        actor: Address,
        actor_collateral: Address,
        sequence: u64,
        quantity: u64,
    ) -> Instruction {
        let mut metas = self.state_prefix(actor);
        metas.extend([
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.policy, false),
            AccountMeta::new_readonly(self.collateral_mint, false),
            AccountMeta::new(actor_collateral, false),
            AccountMeta::new_readonly(self.hoard_authority, false),
            AccountMeta::new(self.hoard_token, false),
        ]);
        metas.extend(
            self.outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(*mint, false)),
        );
        assert_eq!(
            metas.len(),
            seam::ACCOUNT_PREFIX_COLLATERAL + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::Split {
                    market: self.market_id,
                    owner: Hash32::from_bytes(actor.to_bytes()),
                    quantity,
                },
            ),
            metas,
        )
    }

    fn materialize(
        &self,
        actor: Address,
        holder: Address,
        sequence: u64,
        outcome: u8,
        quantity: u64,
    ) -> Instruction {
        let mut metas = self.state_prefix(actor);
        metas.push(AccountMeta::new_readonly(TOKEN_2022, false));
        metas.push(AccountMeta::new(holder, false));
        metas.extend(self.outcome_mints.iter().enumerate().map(|(index, mint)| {
            if index == usize::from(outcome) {
                AccountMeta::new(*mint, false)
            } else {
                AccountMeta::new_readonly(*mint, false)
            }
        }));
        assert_eq!(
            metas.len(),
            seam::ACCOUNT_PREFIX_OUTCOME + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::Materialize {
                    market: self.market_id,
                    owner: Hash32::from_bytes(actor.to_bytes()),
                    destination: Hash32::from_bytes(holder.to_bytes()),
                    outcome,
                    quantity,
                },
            ),
            metas,
        )
    }

    fn resolve(&self, actor: Address) -> Instruction {
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&0_u64.to_le_bytes());
        data.push(1); // ACTION_RESOLVE
        data.push(PAYOUT_INDEX_UNRESOLVED);
        let mut metas = vec![
            AccountMeta::new_readonly(actor, true),
            AccountMeta::new(self.market, false),
            AccountMeta::new_readonly(self.hoard, false),
            AccountMeta::new(self.kernel, false),
            AccountMeta::new(self.supply, false),
            AccountMeta::new_readonly(self.terms, false),
            AccountMeta::new(self.resolution, false),
            AccountMeta::new_readonly(self.feed, false),
            AccountMeta::new_readonly(self.source_spec, false),
            AccountMeta::new_readonly(self.source_archive, false),
            AccountMeta::new_readonly(BUFFER_ACCOUNT, false),
        ];
        metas.extend(
            self.outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(*mint, false)),
        );
        assert_eq!(
            metas.len(),
            observe_resolve::RESOLVE_ACCOUNT_PREFIX + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    fn redeem_internal(
        &self,
        actor: Address,
        actor_collateral: Address,
        sequence: u64,
        outcome: u8,
        quantity: u64,
    ) -> Instruction {
        let mut data = vec![0xd1, 1];
        data.extend_from_slice(&sequence.to_le_bytes());
        data.push(2); // ACTION_REDEEM_INTERNAL
        data.push(outcome);
        data.extend_from_slice(&quantity.to_le_bytes());
        let mut metas = vec![
            AccountMeta::new_readonly(actor, true),
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.hoard, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.kernel, false),
            AccountMeta::new(self.replay, false),
            AccountMeta::new(self.supply, false),
            AccountMeta::new_readonly(self.terms, false),
            AccountMeta::new_readonly(self.resolution, false),
            AccountMeta::new_readonly(self.profile, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.policy, false),
            AccountMeta::new_readonly(self.collateral_mint, false),
            AccountMeta::new(actor_collateral, false),
            AccountMeta::new_readonly(self.hoard_authority, false),
            AccountMeta::new(self.hoard_token, false),
        ];
        metas.extend(
            self.outcome_mints
                .iter()
                .map(|mint| AccountMeta::new_readonly(*mint, false)),
        );
        assert_eq!(
            metas.len(),
            observe_resolve::REDEEM_ACCOUNT_PREFIX + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(PROGRAM_ID, &data, metas)
    }

    fn redeem_external(
        &self,
        claimant: Address,
        source: Address,
        destination: Address,
        quantity: u64,
    ) -> Instruction {
        let mut metas = vec![
            AccountMeta::new_readonly(claimant, true),
            AccountMeta::new_readonly(self.profile, false),
            AccountMeta::new_readonly(self.market, false),
            AccountMeta::new(self.hoard, false),
            AccountMeta::new(self.kernel, false),
            AccountMeta::new(self.supply, false),
            AccountMeta::new_readonly(self.resolution, false),
            AccountMeta::new_readonly(self.terms, false),
            AccountMeta::new_readonly(self.policy, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.collateral_mint, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(self.hoard_authority, false),
            AccountMeta::new(self.hoard_token, false),
            AccountMeta::new(source, false),
        ];
        metas.extend(self.outcome_mints.iter().enumerate().map(|(index, mint)| {
            if index == 0 {
                AccountMeta::new(*mint, false)
            } else {
                AccountMeta::new_readonly(*mint, false)
            }
        }));
        assert_eq!(
            metas.len(),
            external_exit::IX_OUTCOME_MINTS + usize::from(OUTCOMES)
        );
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::RedeemExternal {
                    market: self.market_id,
                    claimant: Hash32::from_bytes(claimant.to_bytes()),
                    source: Hash32::from_bytes(source.to_bytes()),
                    destination: Hash32::from_bytes(destination.to_bytes()),
                    outcome: 0,
                    quantity,
                },
            ),
            metas,
        )
    }

    fn withdraw(
        &self,
        actor: Address,
        destination: Address,
        sequence: u64,
        amount: u64,
    ) -> Instruction {
        let metas = vec![
            AccountMeta::new_readonly(actor, true),
            AccountMeta::new_readonly(self.market, false),
            AccountMeta::new_readonly(self.hoard, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.replay, false),
            AccountMeta::new_readonly(self.profile, false),
            AccountMeta::new_readonly(self.policy, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.collateral_mint, false),
            AccountMeta::new(destination, false),
            AccountMeta::new_readonly(self.hoard_authority, false),
            AccountMeta::new(self.hoard_token, false),
        ];
        assert_eq!(metas.len(), cash_exit::ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                sequence,
                Intent::WithdrawCash {
                    market: self.market_id,
                    owner: Hash32::from_bytes(actor.to_bytes()),
                    destination: Hash32::from_bytes(destination.to_bytes()),
                    amount,
                },
            ),
            metas,
        )
    }
}

async fn create_collateral_plane(
    bank: &mut ProgramTestContext,
    mint: &Keypair,
    actor_token: &Keypair,
    bearer_token: &Keypair,
    actor: Address,
    bearer: Address,
) {
    let rent = bank.banks_client.get_rent().await.unwrap();
    let payer = bank.payer.pubkey();
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
        ],
        &[mint],
    )
    .await;
    for (token, owner) in [(actor_token, actor), (bearer_token, bearer)] {
        succeed(
            bank,
            &[
                system_instruction::create_account(
                    &payer,
                    &token.pubkey(),
                    rent.minimum_balance(TokenAccount::LEN),
                    TokenAccount::LEN as u64,
                    &TOKEN_2022,
                ),
                token_instruction::initialize_account3(
                    &TOKEN_2022,
                    &token.pubkey(),
                    &mint.pubkey(),
                    &owner,
                )
                .unwrap(),
            ],
            &[token],
        )
        .await;
    }
    succeed(
        bank,
        &[
            token_instruction::mint_to(
                &TOKEN_2022,
                &mint.pubkey(),
                &actor_token.pubkey(),
                &payer,
                &[],
                SETS,
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
        &[],
    )
    .await;
}

async fn create_token_account(
    bank: &mut ProgramTestContext,
    account: &Keypair,
    mint: Address,
    owner: Address,
) {
    let rent = bank.banks_client.get_rent().await.unwrap();
    let payer = bank.payer.pubkey();
    succeed(
        bank,
        &[
            system_instruction::create_account(
                &payer,
                &account.pubkey(),
                rent.minimum_balance(TokenAccount::LEN),
                TokenAccount::LEN as u64,
                &TOKEN_2022,
            ),
            token_instruction::initialize_account3(&TOKEN_2022, &account.pubkey(), &mint, &owner)
                .unwrap(),
        ],
        &[account],
    )
    .await;
}

/// Public artifact/Realm/Profile prefix: everything a wallet seals before the
/// market exists. Every Clutch-owned account this constructs is created by an
/// ordinary signed transaction.
async fn prepare_founding(
    bank: &mut ProgramTestContext,
    collateral_mint: Address,
    actor: Address,
    degree: u8,
    feed_id: Hash32,
    start_bucket: u64,
    end_bucket: u64,
) -> Founding {
    let policy_value = fixture_policy(collateral_mint.to_bytes());
    let policy_digest = policy_value.digest().unwrap();
    let profile_id = ParentProfile::from_policy(&policy_value)
        .and_then(|parent| parent.identity())
        .unwrap();
    let policy = upload(
        bank,
        ArtifactKind::CollateralPolicy,
        profile_id,
        policy_digest,
        &policy_value.canonical_bytes().unwrap(),
    )
    .await;

    let realm_id = canonical_realm_id(profile_id, REALM_NONCE);
    let realm = derive(&[seeds::SEED_REALM, &realm_id.bytes()]).0;
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
                        profile_version: 1,
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
    let profile = derive(&[seeds::SEED_PROFILE, &realm_id.bytes(), &profile_id.bytes()]).0;
    succeed(
        bank,
        &[
            budget(),
            Instruction::new_with_bytes(
                PROGRAM_ID,
                &layout_request(
                    0,
                    Intent::InitProfile {
                        realm: realm_id,
                        collateral_policy_digest: policy_digest,
                        subfield_schema_version: policy_value.schema_version,
                        profile_version: 1,
                    },
                ),
                vec![
                    AccountMeta::new(payer, true),
                    AccountMeta::new(profile, false),
                    AccountMeta::new_readonly(realm, false),
                    AccountMeta::new_readonly(policy, false),
                    AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                    AccountMeta::new_readonly(RENT_SYSVAR, false),
                ],
            ),
        ],
        &[],
    )
    .await;

    let mut ticks = [0_u64; MAX_GRID_TICKS];
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
    grid_value.stored_bump = final_address(ArtifactKind::PriceGrid, realm_id, grid_value.grid).1;
    let grid_body = encode(account_len::PRICE_GRID, |out| grid_value.encode(out));
    let _grid = upload(
        bank,
        ArtifactKind::PriceGrid,
        realm_id,
        grid_value.grid,
        &grid_body,
    )
    .await;

    /* Degree-d smooth Terms: the exact payout/knot shape of the admitted
     * native campaign, joined to the PUBLIC mock-source window: one-second
     * buckets, the two-bucket window [start, end), and maturity exactly one
     * bucket past the exclusive end, as the source-ingest gate requires. */
    let mut terms_value = fixture_terms(realm_id, profile_id, feed_id);
    terms_value.price_grid = grid_value.grid;
    terms_value.outcome_count = OUTCOMES;
    terms_value.payout_count = OUTCOMES;
    terms_value.payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    for outcome in 0..usize::from(OUTCOMES) {
        let mut weights = [0_u64; MAX_OUTCOMES];
        weights[outcome] = DENOMINATOR;
        terms_value.payouts[outcome] = PayoutVectorBytes {
            denominator: DENOMINATOR,
            weights,
        };
    }
    terms_value.basis_degree = degree;
    terms_value.knot_count = OUTCOMES + 1 - degree;
    terms_value.uniform_log2_spacing = 3;
    terms_value.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    terms_value.knots = [0; MAX_KNOTS];
    for (index, knot) in terms_value
        .knots
        .iter_mut()
        .take(usize::from(terms_value.knot_count))
        .enumerate()
    {
        *knot = if degree == 1 {
            (index as u128 + 1) * 8
        } else {
            index as u128 * 8
        };
    }
    terms_value.grid_family_id = 5;
    terms_value.grid_version = 2;
    terms_value.bucket_seconds = 1;
    terms_value.expected_start_bucket = start_bucket;
    terms_value.expected_end_bucket_exclusive = end_bucket;
    terms_value.maturity_horizon_buckets = end_bucket - start_bucket + 1;
    terms_value.terms = Hash32::ZERO;
    terms_value.terms = terms_value.recomputed_terms_digest().unwrap();
    terms_value.stored_bump = final_address(ArtifactKind::Terms, realm_id, terms_value.terms).1;
    let terms_id = terms_value.terms;
    let terms_body = encode(account_len::TERMS, |out| terms_value.encode(out));
    let terms = upload(bank, ArtifactKind::Terms, realm_id, terms_id, &terms_body).await;

    let feed_identity =
        FeedIdentity::new(MOCK_ADAPTER, feed_id.bytes(), 7, 1).expect("walk feed identity");
    let window = WindowDomain::new(
        feed_identity,
        Grid::new(5, 2, 1).expect("walk grid"),
        start_bucket,
        end_bucket,
        end_bucket + 1,
        0,
        CoveragePolicy::COMPLETE_REQUIRED,
    )
    .expect("walk window domain");
    let window_id = canonical_window_id(window);

    let market_id = canonical_market_id(realm_id, profile_id, MARKET_NONCE);
    let owner = actor.to_bytes();
    let market = derive(&[seeds::SEED_MARKET, &realm_id.bytes(), &market_id.bytes()]).0;
    let hoard = derive(&[seeds::SEED_HOARD, &market_id.bytes()]).0;
    let position = derive(&[seeds::SEED_POSITION, &market_id.bytes(), &owner]).0;
    let kernel = derive(&[seeds::SEED_KERNEL, &market_id.bytes()]).0;
    let replay = derive(&[
        seeds::SEED_REPLAY,
        &market_id.bytes(),
        &owner,
        &0_u64.to_le_bytes(),
    ])
    .0;
    let supply = derive(&[seeds::SEED_SUPPLY, &market_id.bytes()]).0;
    let resolution = derive(&[seeds::SEED_RESOLUTION, &market_id.bytes()]).0;
    let hoard_authority = derive(&[seeds::SEED_HOARD_AUTHORITY, &market_id.bytes()]).0;
    let hoard_token = derive(&[seeds::SEED_HOARD_TOKEN, &market_id.bytes()]).0;
    let outcome_mints = [0_u8, 1, 2, 3]
        .map(|outcome| derive(&[seeds::SEED_OUTCOME_MINT, &market_id.bytes(), &[outcome]]).0);
    let feed = derive(&[seeds::SEED_FEED, &feed_id.bytes()]).0;
    let source_spec = derive(&[seeds::SEED_SOURCE_SPEC, &feed_id.bytes()]).0;
    let source_archive = derive(&[
        seeds::SEED_SOURCE_ARCHIVE,
        &feed_id.bytes(),
        &window_id.bytes(),
    ])
    .0;

    Founding {
        realm_id,
        profile_id,
        terms_id,
        feed_id,
        policy,
        realm,
        profile,
        terms,
        market_id,
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
        collateral_mint,
        feed,
        source_spec,
        source_archive,
        window_id,
        start_bucket,
        end_bucket,
    }
}

async fn assert_blank_market_targets_absent(bank: &mut ProgramTestContext, f: &Founding) {
    for target in [
        f.market,
        f.hoard,
        f.position,
        f.kernel,
        f.replay,
        f.supply,
        f.resolution,
        f.hoard_token,
    ]
    .into_iter()
    .chain(f.outcome_mints)
    {
        assert!(get(bank, target).await.is_none(), "{target}");
    }
}

async fn assert_blank_bank_reload(bank: &mut ProgramTestContext, f: &Founding, degree: u8) {
    let market = MarketAccount::decode(&existing(bank, f.market).await.data).unwrap();
    assert_eq!(market.lifecycle, 0);
    assert_eq!(market.outcome_count, OUTCOMES);
    let kernel_bytes = existing(bank, f.kernel).await.data;
    assert_eq!(kernel_bytes.len(), KERNEL_ACCOUNT_LEN);
    let kernel = KernelAccount::decode(&kernel_bytes).unwrap();
    assert_eq!(kernel.basis_mode, BasisMode::DerivedBasis);
    assert_eq!(kernel.phase, 0);
    assert_eq!(kernel.total_supply, [0; KERNEL_MAX_OUTCOMES]);
    let resolution_bytes = existing(bank, f.resolution).await.data;
    assert_eq!(resolution_bytes.len(), NATIVE_RESOLUTION_LEN);
    let resolution = NativeResolutionAccount::decode(&resolution_bytes).unwrap();
    assert!(!resolution.is_resolved());
    assert_eq!(resolution.market, f.market_id);
    let terms = TermsAccount::decode(&existing(bank, f.terms).await.data).unwrap();
    assert_eq!(terms.basis_degree, degree);
    assert_eq!(terms.terms, f.terms_id);
}

/// Write the next mock provider record. This stands in for the laboratory
/// provider program's own account write; it is host-driven state, named in
/// the module inventory, and it flows into Clutch only through the public
/// authenticated append/seal instructions.
fn write_provider_record(
    bank: &mut ProgramTestContext,
    bucket: u64,
    sequence: u64,
    slot: u64,
    time: u64,
    price: u128,
) {
    bank.set_account(
        &MOCK_SOURCE,
        &injected(
            MOCK_PROGRAM,
            provider_record(bucket, sequence, slot, time, price),
            false,
        ),
    );
}

struct Scenario {
    bank: ProgramTestContext,
    actor: Keypair,
    bearer: Keypair,
    actor_collateral: Address,
    bearer_collateral: Address,
    founding: Founding,
    spec: SourceSpecV1,
    clock_slot: u64,
    point: u128,
}

impl Scenario {
    async fn start(degree: u8) -> Self {
        let actor = actor_keypair();
        let bearer = bearer_keypair();
        let mint = collateral_mint_keypair();
        let (point, _, _) = point_and_weights(degree);
        let spec = walk_spec();
        let feed_id = spec.feed_id();
        let mock = cfg!(feature = "non-production-mock-source");

        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);
        /* Ordinary wallet funding: the local equivalent of an airdrop. */
        for wallet in [actor.pubkey(), bearer.pubkey()] {
            test.add_account(
                wallet,
                Account {
                    lamports: 20_000_000_000,
                    data: vec![],
                    owner: SYSTEM_PROGRAM,
                    executable: false,
                    rent_epoch: 0,
                },
            );
        }
        if mock {
            /* Injected prerequisites 1-3 of the mock campaign: the NON-Clutch
             * laboratory provider the compiled mock registry names. */
            test.add_account(
                MOCK_PROGRAM,
                injected(MOCK_PROGRAM_OWNER, b"MOCK-PROVIDER-V1".to_vec(), true).into(),
            );
            let mut deployment = b"DEP1".to_vec();
            deployment.extend_from_slice(&DEPLOYMENT_GENERATION.to_le_bytes());
            test.add_account(
                MOCK_DEPLOYMENT,
                injected(MOCK_DEPLOYMENT_OWNER, deployment, false).into(),
            );
            test.add_account(
                MOCK_SOURCE,
                injected(MOCK_PROGRAM, vec![0; 77], false).into(),
            );
        }
        let mut bank = test.start_with_context().await;
        let (clock_slot, unix) = clock(&existing(&mut bank, CLOCK_SYSVAR).await);
        let start_bucket = unix.saturating_sub(2);
        let end_bucket = unix;

        let actor_collateral = actor_collateral_keypair();
        let bearer_collateral = bearer_collateral_keypair();
        create_collateral_plane(
            &mut bank,
            &mint,
            &actor_collateral,
            &bearer_collateral,
            actor.pubkey(),
            bearer.pubkey(),
        )
        .await;

        let founding = prepare_founding(
            &mut bank,
            mint.pubkey(),
            actor.pubkey(),
            degree,
            feed_id,
            start_bucket,
            end_bucket,
        )
        .await;
        assert!(get(&mut bank, founding.source_spec).await.is_none());
        assert!(get(&mut bank, founding.feed).await.is_none());
        assert!(get(&mut bank, founding.source_archive).await.is_none());
        assert_blank_market_targets_absent(&mut bank, &founding).await;

        Self {
            bank,
            actor,
            bearer,
            actor_collateral: actor_collateral.pubkey(),
            bearer_collateral: bearer_collateral.pubkey(),
            founding,
            spec,
            clock_slot,
            point,
        }
    }

    async fn token_amount(&mut self, address: Address) -> u64 {
        StateWithExtensions::<TokenAccount>::unpack(&existing(&mut self.bank, address).await.data)
            .expect("Token-2022 account reload")
            .base
            .amount
    }

    async fn mint_supply(&mut self, address: Address) -> u64 {
        StateWithExtensions::<Mint>::unpack(&existing(&mut self.bank, address).await.data)
            .expect("Token-2022 mint reload")
            .base
            .supply
    }
}

/// The public create/append/seal source route on the mock ELF: SourceSpec and
/// Feed, then the exact-window archive, two authenticated appends covering
/// the complete window, and the maturity-authenticating seal that advances
/// the feed head. Returns the (init_spec, init_archive, appends, seal) CU.
async fn public_source_route(scenario: &mut Scenario) -> (u64, u64, [u64; 2], u64) {
    let f = scenario.founding.clone();
    let payer = scenario.bank.payer.pubkey();
    let start = f.start_bucket;
    let end = f.end_bucket;
    let slot = scenario.clock_slot;
    let point = scenario.point;

    write_provider_record(
        &mut scenario.bank,
        start,
        1,
        slot.saturating_sub(2),
        start,
        point,
    );
    let init_spec_cu = succeed(
        &mut scenario.bank,
        &[budget(), f.init_spec(payer, scenario.spec)],
        &[],
    )
    .await;
    let spec_account = existing(&mut scenario.bank, f.source_spec).await;
    assert_eq!(spec_account.owner, PROGRAM_ID);
    assert_eq!(spec_account.data.len(), SOURCE_SPEC_ACCOUNT_V1_BYTES);
    let feed = FeedAccount::decode(&existing(&mut scenario.bank, f.feed).await.data).unwrap();
    assert_eq!(feed.feed, f.feed_id);
    assert_eq!(feed.cursor, start);
    assert_eq!(feed.archive_pages, 0);

    let init_archive_cu =
        succeed(&mut scenario.bank, &[budget(), f.init_archive(payer)], &[]).await;

    let append0_cu = succeed(
        &mut scenario.bank,
        &[budget(), f.mutate_archive(0, false)],
        &[],
    )
    .await;
    write_provider_record(
        &mut scenario.bank,
        start + 1,
        2,
        slot.saturating_sub(1),
        start + 1,
        point,
    );
    let append1_cu = succeed(
        &mut scenario.bank,
        &[budget(), f.mutate_archive(1, false)],
        &[],
    )
    .await;
    write_provider_record(&mut scenario.bank, end, 3, slot, end, point);
    let seal_cu = succeed(
        &mut scenario.bank,
        &[budget(), f.mutate_archive(2, true)],
        &[],
    )
    .await;
    let feed = FeedAccount::decode(&existing(&mut scenario.bank, f.feed).await.data).unwrap();
    assert_eq!(feed.cursor, end + 1);
    assert_eq!(feed.archive_pages, 1);

    (
        init_spec_cu,
        init_archive_cu,
        [append0_cu, append1_cu],
        seal_cu,
    )
}

/// The funded value segment on the NON-PRODUCTION mock-source ELF.
async fn funded_segment(scenario: &mut Scenario, degree: u8) {
    let (point, expected_weights, external_lot) = point_and_weights(degree);
    let f = scenario.founding.clone();
    let actor = scenario.actor.insecure_clone();
    let bearer = scenario.bearer.insecure_clone();

    let (init_spec_cu, init_archive_cu, append_cu, seal_cu) = public_source_route(scenario).await;

    let create_units = succeed(
        &mut scenario.bank,
        &[budget(), f.create_market(actor.pubkey())],
        &[&actor],
    )
    .await;
    assert_blank_bank_reload(&mut scenario.bank, &f, degree).await;

    let actor_outcome = actor_outcome_keypair();
    let bearer_outcome = bearer_outcome_keypair();
    create_token_account(
        &mut scenario.bank,
        &actor_outcome,
        f.outcome_mints[0],
        actor.pubkey(),
    )
    .await;
    create_token_account(
        &mut scenario.bank,
        &bearer_outcome,
        f.outcome_mints[0],
        bearer.pubkey(),
    )
    .await;
    let actor_outcome = actor_outcome.pubkey();
    let bearer_outcome = bearer_outcome.pubkey();

    /* Injected prerequisite 4: the program-owned redundant evidence buffer.
     * Its every byte is checked against the publicly sealed archive inside
     * Resolve; a projection that disagrees with the archive refuses. */
    let buffer = evidence_buffer_bytes(f.window_id, f.feed_id, f.start_bucket, f.end_bucket, point);
    scenario
        .bank
        .set_account(&BUFFER_ACCOUNT, &injected(PROGRAM_ID, buffer, false));

    let endow_units = succeed(
        &mut scenario.bank,
        &[
            budget(),
            f.endow(actor.pubkey(), scenario.actor_collateral, 0, SETS),
        ],
        &[&actor],
    )
    .await;
    let position =
        PositionAccount::decode(&existing(&mut scenario.bank, f.position).await.data).unwrap();
    assert_eq!(position.cash_atoms, SETS);
    assert_eq!(scenario.token_amount(scenario.actor_collateral).await, 0);
    assert_eq!(scenario.token_amount(f.hoard_token).await, SETS);

    let split_units = succeed(
        &mut scenario.bank,
        &[
            budget(),
            f.split(actor.pubkey(), scenario.actor_collateral, 1, SETS),
        ],
        &[&actor],
    )
    .await;
    let position =
        PositionAccount::decode(&existing(&mut scenario.bank, f.position).await.data).unwrap();
    assert_eq!(position.cash_atoms, 0);
    assert_eq!(position.internal[..4], [SETS; 4]);
    let kernel = KernelAccount::decode(&existing(&mut scenario.bank, f.kernel).await.data).unwrap();
    assert_eq!(kernel.basis_mode, BasisMode::DerivedBasis);
    assert_eq!(kernel.total_supply[..4], [SETS; 4]);
    let supply =
        SupplyLedgerAccount::decode(&existing(&mut scenario.bank, f.supply).await.data).unwrap();
    assert_eq!(supply.internal_supply[..4], [SETS; 4]);
    assert_eq!(supply.external_supply[..4], [0; 4]);
    assert_eq!(
        HoardAccount::decode(&existing(&mut scenario.bank, f.hoard).await.data)
            .unwrap()
            .collateral_atoms,
        SETS
    );

    let materialize_units = succeed(
        &mut scenario.bank,
        &[
            budget(),
            f.materialize(actor.pubkey(), actor_outcome, 2, 0, external_lot),
        ],
        &[&actor],
    )
    .await;
    assert_eq!(scenario.token_amount(actor_outcome).await, external_lot);
    assert_eq!(scenario.mint_supply(f.outcome_mints[0]).await, external_lot);

    succeed(
        &mut scenario.bank,
        &[token_instruction::transfer_checked(
            &TOKEN_2022,
            &actor_outcome,
            &f.outcome_mints[0],
            &bearer_outcome,
            &actor.pubkey(),
            &[],
            external_lot,
            0,
        )
        .unwrap()],
        &[&actor],
    )
    .await;
    assert_eq!(scenario.token_amount(actor_outcome).await, 0);
    assert_eq!(scenario.token_amount(bearer_outcome).await, external_lot);

    let resolve_units = succeed(
        &mut scenario.bank,
        &[budget(), f.resolve(actor.pubkey())],
        &[&actor],
    )
    .await;
    let resolution_bytes = existing(&mut scenario.bank, f.resolution).await.data;
    assert_eq!(resolution_bytes.len(), NATIVE_RESOLUTION_LEN);
    let resolution = NativeResolutionAccount::decode(&resolution_bytes).unwrap();
    assert_eq!(resolution.mode, RESOLUTION_MODE_DERIVED_POINT);
    assert_eq!(resolution.payout_index, PAYOUT_INDEX_UNRESOLVED);
    assert_eq!(resolution.resolved_value, point);
    assert_eq!(resolution.vector.denominator, DENOMINATOR);
    assert_eq!(resolution.vector.weights, expected_weights);
    assert_eq!(resolution.window, f.window_id);
    let resolved_market =
        MarketAccount::decode(&existing(&mut scenario.bank, f.market).await.data).unwrap();
    assert_eq!(resolved_market.lifecycle, 1);
    let resolved_kernel =
        KernelAccount::decode(&existing(&mut scenario.bank, f.kernel).await.data).unwrap();
    assert_eq!(resolved_kernel.phase, 1);
    assert_eq!(resolved_kernel.basis_mode, BasisMode::DerivedBasis);

    let external_units = succeed(
        &mut scenario.bank,
        &[
            budget(),
            f.redeem_external(
                bearer.pubkey(),
                bearer_outcome,
                scenario.bearer_collateral,
                external_lot,
            ),
        ],
        &[&bearer],
    )
    .await;
    let external_payout = expected_weights[0] * external_lot / DENOMINATOR;
    assert_eq!(external_payout, 1);
    assert_eq!(scenario.token_amount(bearer_outcome).await, 0);
    assert_eq!(scenario.mint_supply(f.outcome_mints[0]).await, 0);
    assert_eq!(
        scenario.token_amount(scenario.bearer_collateral).await,
        external_payout
    );
    assert_eq!(
        existing(&mut scenario.bank, f.resolution).await.data,
        resolution_bytes
    );

    let mut sequence = 3_u64;
    let mut internal_payout = 0_u64;
    let mut redeem_units = [0_u64; 4];
    for outcome in 0..OUTCOMES {
        let quantity = if outcome == 0 {
            SETS - external_lot
        } else {
            SETS
        };
        let payout = quantity * expected_weights[usize::from(outcome)] / DENOMINATOR;
        internal_payout += payout;
        redeem_units[usize::from(outcome)] = succeed(
            &mut scenario.bank,
            &[
                budget(),
                f.redeem_internal(
                    actor.pubkey(),
                    scenario.actor_collateral,
                    sequence,
                    outcome,
                    quantity,
                ),
            ],
            &[&actor],
        )
        .await;
        sequence += 1;
        assert_eq!(
            existing(&mut scenario.bank, f.resolution).await.data,
            resolution_bytes
        );
    }
    assert_eq!(internal_payout + external_payout, SETS);

    let position =
        PositionAccount::decode(&existing(&mut scenario.bank, f.position).await.data).unwrap();
    assert_eq!(position.internal[..4], [0; 4]);
    assert_eq!(position.cash_atoms, internal_payout);
    let supply =
        SupplyLedgerAccount::decode(&existing(&mut scenario.bank, f.supply).await.data).unwrap();
    assert_eq!(supply.internal_supply[..4], [0; 4]);
    assert_eq!(supply.external_supply[..4], [0; 4]);
    let kernel = KernelAccount::decode(&existing(&mut scenario.bank, f.kernel).await.data).unwrap();
    assert_eq!(kernel.total_supply[..4], [0; 4]);
    assert_eq!(
        HoardAccount::decode(&existing(&mut scenario.bank, f.hoard).await.data)
            .unwrap()
            .collateral_atoms,
        0
    );
    assert_eq!(scenario.token_amount(f.hoard_token).await, internal_payout);

    let withdraw_units = succeed(
        &mut scenario.bank,
        &[
            budget(),
            f.withdraw(
                actor.pubkey(),
                scenario.actor_collateral,
                sequence,
                internal_payout,
            ),
        ],
        &[&actor],
    )
    .await;
    let position =
        PositionAccount::decode(&existing(&mut scenario.bank, f.position).await.data).unwrap();
    assert_eq!(position.cash_atoms, 0);
    assert_eq!(position.reserved_cash_atoms, 0);
    let replay = ReplayAccount::decode(&existing(&mut scenario.bank, f.replay).await.data).unwrap();
    assert_eq!(replay.sequence, sequence + 1);
    assert_eq!(scenario.token_amount(f.hoard_token).await, 0);
    assert_eq!(
        scenario.token_amount(scenario.actor_collateral).await,
        internal_payout
    );
    assert_eq!(
        scenario.token_amount(scenario.actor_collateral).await
            + scenario.token_amount(scenario.bearer_collateral).await,
        SETS
    );
    assert_eq!(
        existing(&mut scenario.bank, f.resolution).await.data,
        resolution_bytes
    );

    println!(
        "NON-PRODUCTION mock-source joined d{degree} point={point}: \
         InitSourceSpec {init_spec_cu}, InitSourceArchive {init_archive_cu}, \
         Append {append_cu:?}, Seal {seal_cu}, CreateMarket {create_units}, \
         Endow {endow_units}, Split {split_units}, Materialize {materialize_units}, \
         Resolve {resolve_units}, ExternalRedeem {external_units}, \
         InternalRedeem {redeem_units:?}, Withdraw {withdraw_units} CU"
    );
}

/// The default empty-registry ELF's asserted value boundary. The public
/// prefix is identical; the walk then proves the boundary is the closed
/// source-release registry and nothing else.
async fn default_refusal_segment(scenario: &mut Scenario, degree: u8) {
    let f = scenario.founding.clone();
    let actor = scenario.actor.insecure_clone();
    let payer = scenario.bank.payer.pubkey();

    let create_units = succeed(
        &mut scenario.bank,
        &[budget(), f.create_market(actor.pubkey())],
        &[&actor],
    )
    .await;
    assert_blank_bank_reload(&mut scenario.bank, &f, degree).await;

    /* Asserted step: the PUBLIC source-construction route refuses with the
     * stable registry code before creating anything. */
    refuse(
        &mut scenario.bank,
        &[budget(), f.init_spec(payer, scenario.spec)],
        &[],
        ClutchError::SourceReleaseUnavailable,
    )
    .await;
    assert!(get(&mut scenario.bank, f.source_spec).await.is_none());
    assert!(get(&mut scenario.bank, f.feed).await.is_none());

    /* Asserted step: Endow with NO SourceSpec at all refuses at the
     * state-role gate (the absent slot is not program-owned), which precedes
     * every source gate. */
    refuse(
        &mut scenario.bank,
        &[
            budget(),
            f.endow(actor.pubkey(), scenario.actor_collateral, 0, SETS),
        ],
        &[&actor],
        ClutchError::WrongProgramOwner,
    )
    .await;

    /* Injected prerequisite 1 (the default campaign's only one): a canonical
     * SourceSpec image, host-encoded by the program's own codec. It exists
     * solely because the public route above refuses on this ELF, so the next
     * refusal is sharpened onto the registry gate itself. */
    let (_, spec_bump) = derive(&[seeds::SEED_SOURCE_SPEC, &f.feed_id.bytes()]);
    let mut spec_image = vec![0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    initialize_source_spec_account(&mut spec_image, scenario.spec, spec_bump)
        .expect("canonical spec image encodes");
    scenario
        .bank
        .set_account(&f.source_spec, &injected(PROGRAM_ID, spec_image, false));

    /* Asserted step: the default ELF refuses Endow with exactly 0x0079 and
     * leaves every watched account byte-identical. The compute-budget limit
     * differs by one unit so this second submission is a distinct transaction
     * rather than a duplicate of the refused one above. */
    let watched = [
        f.market,
        f.hoard,
        f.position,
        f.kernel,
        f.replay,
        f.supply,
        f.resolution,
        f.hoard_token,
        scenario.actor_collateral,
    ];
    let before = snapshot(&mut scenario.bank, &watched).await;
    refuse(
        &mut scenario.bank,
        &[
            Instruction::new_with_bytes(
                COMPUTE_BUDGET,
                &compute_unit_limit_data(1_399_999),
                vec![],
            ),
            f.endow(actor.pubkey(), scenario.actor_collateral, 0, SETS),
        ],
        &[&actor],
        ClutchError::SourceReleaseUnavailable,
    )
    .await;
    assert_eq!(snapshot(&mut scenario.bank, &watched).await, before);
    assert_eq!(scenario.token_amount(f.hoard_token).await, 0);
    assert_eq!(scenario.token_amount(scenario.actor_collateral).await, SETS);

    println!(
        "default-empty-registry joined d{degree}: CreateMarket {create_units} CU; \
         public InitSourceSpec REFUSE Custom(0x0079); \
         Endow(no spec) REFUSE WrongProgramOwner; \
         Endow(injected canonical spec) REFUSE Custom(0x0079), watched bytes identical"
    );
}

async fn joined_walk(degree: u8) {
    let mut scenario = Scenario::start(degree).await;
    if cfg!(feature = "non-production-mock-source") {
        funded_segment(&mut scenario, degree).await;
    } else {
        default_refusal_segment(&mut scenario, degree).await;
    }
}

#[tokio::test]
async fn blank_bank_joined_lifecycle_degree_one() {
    joined_walk(1).await;
}

#[tokio::test]
async fn blank_bank_joined_lifecycle_degree_two() {
    joined_walk(2).await;
}

#[tokio::test]
async fn blank_bank_joined_lifecycle_degree_three() {
    joined_walk(3).await;
}
