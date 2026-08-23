#![cfg(feature = "non-production-mock-source")]

//! Blank-bank, ordinary-wallet lifecycle for native B-spline degrees one to three.
//!
//! The market and every mutable market account are absent at genesis. A wallet
//! creates a real Token-2022 collateral mint and accounts, uploads and seals
//! canonical policy/grid/Terms artifacts, initializes Realm/Profile, and then
//! founds the market through `CreateMarket`. The same wallets drive Endow,
//! Split, Materialize, an ordinary Token-2022 bearer transfer, native Resolve,
//! exact internal and external redemption, and the final cash withdrawal.
//!
//! Source ingestion is deliberately not overstated: the canonical SourceSpec,
//! sealed SourceArchive, Feed head, and redundant buffer are deterministic
//! genesis-assisted mock-provider fixtures because the program has no live
//! provider/parser/archive-construction instruction. Live Resolve nevertheless
//! authenticates their ownership, exact canonical PDAs, sealed receipt, source
//! domain, and redundant byte equality before it persists a native v3 result.

use {
    clutch_kernel::{BasisMode, MAX_OUTCOMES as KERNEL_MAX_OUTCOMES},
    clutch_sbf::{
        instructions::{
            cash_exit, external_exit, genesis, market_init, observe_resolve, split as seam,
        },
        seeds,
    },
    clutch_solana_layout::{
        account_len,
        artifact::{ArtifactKind, ARTIFACT_CHUNK_BYTES},
        canonical_market_id, canonical_realm_id,
        native_resolution::{
            NativeResolutionAccount, NATIVE_RESOLUTION_LEN, RESOLUTION_MODE_DERIVED_POINT,
        },
        FeedAccount, Hash32, HoardAccount, Intent, MarketAccount, PayoutVectorBytes,
        PositionAccount, PriceGridAccount, ProfileAccount, RealmAccount, SupplyLedgerAccount,
        TermsAccount, MAX_GRID_TICKS, MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS,
        PAYOUT_INDEX_UNRESOLVED, PAYOUT_MAP_UNUSED,
    },
    clutch_solana_reference::{KernelAccount, ReplayAccount, KERNEL_ACCOUNT_LEN},
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, fixture_policy, fixture_policy_identity,
        fixture_terms, layout_request, rewrite_plane_source_archive,
        source_resolution_evidence_buffer, token_account_bytes, Mode, PROGRAM_ID, RENT_SYSVAR,
        SYSTEM_PROGRAM, TOKEN_2022,
    },
    solana_account::{Account, AccountSharedData},
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
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
const COMPUTE_BUDGET: Address = Address::new_from_array([
    3, 6, 70, 111, 229, 33, 23, 50, 255, 236, 173, 186, 114, 195, 155, 231, 188, 140, 229, 187,
    197, 247, 18, 107, 44, 67, 155, 58, 64, 0, 0, 0,
]);
const REALM_NONCE: u64 = 7;
const MARKET_NONCE: u64 = 0x92;
const OUTCOMES: u8 = 4;
const SETS: u64 = 64;
const DENOMINATOR: u64 = 64;
const SUBSTITUTE_ARCHIVE: Address = Address::new_from_array([0xd3; 32]);
const OVERFLOW_DESTINATION: Address = Address::new_from_array([0xd5; 32]);

fn actor_keypair() -> Keypair {
    Keypair::new_from_array([0x31; 32])
}

fn bearer_keypair() -> Keypair {
    Keypair::new_from_array([0x32; 32])
}

fn collateral_mint_keypair() -> Keypair {
    Keypair::new_from_array([0x33; 32])
}

fn actor_collateral_keypair() -> Keypair {
    Keypair::new_from_array([0x34; 32])
}

fn bearer_collateral_keypair() -> Keypair {
    Keypair::new_from_array([0x35; 32])
}

fn actor_outcome_keypair() -> Keypair {
    Keypair::new_from_array([0x36; 32])
}

fn bearer_outcome_keypair() -> Keypair {
    Keypair::new_from_array([0x37; 32])
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

async fn get(bank: &mut ProgramTestContext, address: Address) -> Account {
    bank.banks_client
        .get_account(address)
        .await
        .unwrap()
        .expect("account exists")
}

async fn snapshot(bank: &mut ProgramTestContext, addresses: &[Address]) -> Vec<Vec<u8>> {
    let mut out = Vec::with_capacity(addresses.len());
    for address in addresses {
        out.push(get(bank, *address).await.data);
    }
    out
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
    let prefix = match kind {
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
        | ArtifactKind::ProductCapabilityRegistryV2
        | ArtifactKind::CompiledProductSeriesBundleV1) => {
            return derive(&[
                seeds::SEED_PRODUCT_ARTIFACT_V1,
                &[kind.byte()],
                &digest.bytes(),
            ]);
        }
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
    final_account
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
    source_spec: Address,
    source_archive: Address,
    buffer: Address,
    window_id: Hash32,
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
                    feed: self.feed_id,
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

    fn resolve(&self, actor: Address, archive: Address) -> Instruction {
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
            AccountMeta::new_readonly(derive(&[seeds::SEED_FEED, &self.feed_id.bytes()]).0, false),
            AccountMeta::new_readonly(self.source_spec, false),
            AccountMeta::new_readonly(archive, false),
            AccountMeta::new_readonly(self.buffer, false),
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
            AccountMeta::new_readonly(self.realm, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
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

struct Scenario {
    bank: ProgramTestContext,
    actor: Keypair,
    bearer: Keypair,
    actor_collateral: Address,
    bearer_collateral: Address,
    actor_outcome: Address,
    bearer_outcome: Address,
    founding: Founding,
}

impl Scenario {
    async fn start(degree: u8) -> Self {
        let actor = actor_keypair();
        let bearer = bearer_keypair();
        let mint = collateral_mint_keypair();
        let (point, _, _) = point_and_weights(degree);

        let mut source = build_plane(actor.pubkey(), mint.pubkey(), MARKET_NONCE, Mode::Empty);
        rewrite_plane_source_archive(&mut source, point, 0);
        source
            .accounts
            .iter_mut()
            .find(|account| account.address == source.buffer)
            .expect("fixture buffer")
            .data =
            source_resolution_evidence_buffer(source.window_id, source.feed_id, point, point);

        let feed_account = source
            .accounts
            .iter()
            .find(|account| account.address == source.feed.address)
            .expect("fixture feed")
            .clone();
        let source_spec_account = source
            .accounts
            .iter()
            .find(|account| account.address == source.source_spec.address)
            .expect("fixture source spec")
            .clone();
        let source_archive_account = source
            .accounts
            .iter()
            .find(|account| account.address == source.source_archive.address)
            .expect("fixture source archive")
            .clone();
        let buffer_account = source
            .accounts
            .iter()
            .find(|account| account.address == source.buffer)
            .expect("fixture buffer")
            .clone();

        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);
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
        for account in [
            &feed_account,
            &source_spec_account,
            &source_archive_account,
            &buffer_account,
        ] {
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
                lamports: Rent::default()
                    .minimum_balance(source_archive_account.data.len())
                    .max(1),
                data: source_archive_account.data.clone(),
                owner: PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        let overflow = token_account_bytes(mint.pubkey(), bearer.pubkey(), u64::MAX);
        test.add_account(
            OVERFLOW_DESTINATION,
            Account {
                lamports: Rent::default().minimum_balance(overflow.len()).max(1),
                data: overflow,
                owner: TOKEN_2022,
                executable: false,
                rent_epoch: 0,
            },
        );
        let mut bank = test.start_with_context().await;

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

        let founding =
            prepare_founding(&mut bank, &source, mint.pubkey(), actor.pubkey(), degree).await;
        let create_units = succeed(
            &mut bank,
            &[budget(), founding.create_market(actor.pubkey())],
            &[&actor],
        )
        .await;
        assert_blank_bank_reload(&mut bank, &founding, degree).await;

        let actor_outcome = actor_outcome_keypair();
        let bearer_outcome = bearer_outcome_keypair();
        create_token_account(
            &mut bank,
            &actor_outcome,
            founding.outcome_mints[0],
            actor.pubkey(),
        )
        .await;
        create_token_account(
            &mut bank,
            &bearer_outcome,
            founding.outcome_mints[0],
            bearer.pubkey(),
        )
        .await;
        println!("native full d{degree}: CreateMarket {create_units} CU");

        Self {
            bank,
            actor,
            bearer,
            actor_collateral: actor_collateral.pubkey(),
            bearer_collateral: bearer_collateral.pubkey(),
            actor_outcome: actor_outcome.pubkey(),
            bearer_outcome: bearer_outcome.pubkey(),
            founding,
        }
    }

    async fn token_amount(&mut self, address: Address) -> u64 {
        StateWithExtensions::<TokenAccount>::unpack(&get(&mut self.bank, address).await.data)
            .expect("Token-2022 account reload")
            .base
            .amount
    }

    async fn mint_supply(&mut self, address: Address) -> u64 {
        StateWithExtensions::<Mint>::unpack(&get(&mut self.bank, address).await.data)
            .expect("Token-2022 mint reload")
            .base
            .supply
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

async fn prepare_founding(
    bank: &mut ProgramTestContext,
    source: &clutch_svm_fixture::Plane,
    collateral_mint: Address,
    actor: Address,
    degree: u8,
) -> Founding {
    let policy_value = fixture_policy(collateral_mint.to_bytes());
    let (policy_digest, release_id, profile_id) = fixture_policy_identity(policy_value);
    let policy = upload(
        bank,
        ArtifactKind::CollateralPolicy,
        profile_id,
        policy_digest,
        &policy_value.encode().unwrap(),
    )
    .await;

    let realm_id = canonical_realm_id(profile_id, REALM_NONCE);
    assert_eq!(
        realm_id, source.realm_id,
        "source Feed belongs to this Realm"
    );
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
    let profile = derive(&[seeds::SEED_PROFILE, &realm_id.bytes(), &profile_id.bytes()]).0;
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

    let mut terms_value = fixture_terms(realm_id, profile_id, source.feed_id);
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
    terms_value.terms = Hash32::ZERO;
    terms_value.terms = terms_value.recomputed_terms_digest().unwrap();
    terms_value.stored_bump = final_address(ArtifactKind::Terms, realm_id, terms_value.terms).1;
    let terms_id = terms_value.terms;
    let terms_body = encode(account_len::TERMS, |out| terms_value.encode(out));
    let terms = upload(bank, ArtifactKind::Terms, realm_id, terms_id, &terms_body).await;

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

    Founding {
        realm_id,
        profile_id,
        terms_id,
        feed_id: source.feed_id,
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
        source_spec: source.source_spec.address,
        source_archive: source.source_archive.address,
        buffer: source.buffer,
        window_id: source.window_id,
    }
}

async fn assert_blank_bank_reload(bank: &mut ProgramTestContext, f: &Founding, degree: u8) {
    assert!(RealmAccount::decode(&get(bank, f.realm).await.data).is_ok());
    assert!(ProfileAccount::decode(&get(bank, f.profile).await.data).is_ok());
    let market = MarketAccount::decode(&get(bank, f.market).await.data).unwrap();
    assert_eq!(market.lifecycle, 0);
    assert_eq!(market.outcome_count, OUTCOMES);
    let kernel_bytes = get(bank, f.kernel).await.data;
    assert_eq!(kernel_bytes.len(), KERNEL_ACCOUNT_LEN);
    let kernel = KernelAccount::decode(&kernel_bytes).unwrap();
    assert_eq!(kernel.basis_mode, BasisMode::DerivedBasis);
    assert_eq!(kernel.phase, 0);
    assert_eq!(kernel.total_supply, [0; KERNEL_MAX_OUTCOMES]);
    let resolution_bytes = get(bank, f.resolution).await.data;
    assert_eq!(resolution_bytes.len(), NATIVE_RESOLUTION_LEN);
    let resolution = NativeResolutionAccount::decode(&resolution_bytes).unwrap();
    assert!(!resolution.is_resolved());
    assert_eq!(resolution.market, f.market_id);
    let terms = TermsAccount::decode(&get(bank, f.terms).await.data).unwrap();
    assert_eq!(terms.basis_degree, degree);
    assert_eq!(terms.terms, f.terms_id);
    let feed = derive(&[seeds::SEED_FEED, &f.feed_id.bytes()]).0;
    assert_eq!(
        FeedAccount::decode(&get(bank, feed).await.data)
            .unwrap()
            .feed,
        f.feed_id
    );
}

async fn set_kernel_mode(
    bank: &mut ProgramTestContext,
    address: Address,
    mode: BasisMode,
) -> Account {
    let original = get(bank, address).await;
    let mut kernel = KernelAccount::decode(&original.data).unwrap();
    kernel.basis_mode = mode;
    let mut hostile = original.clone();
    hostile.data = encode(KERNEL_ACCOUNT_LEN, |out| kernel.encode(out));
    bank.set_account(&address, &AccountSharedData::from(hostile));
    original
}

async fn run_degree(degree: u8) {
    let (point, expected_weights, external_lot) = point_and_weights(degree);
    let mut scenario = Scenario::start(degree).await;
    let f = scenario.founding.clone();
    let actor = scenario.actor.insecure_clone();
    let bearer = scenario.bearer.insecure_clone();

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
        PositionAccount::decode(&get(&mut scenario.bank, f.position).await.data).unwrap();
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
        PositionAccount::decode(&get(&mut scenario.bank, f.position).await.data).unwrap();
    assert_eq!(position.cash_atoms, 0);
    assert_eq!(position.internal[..4], [SETS; 4]);
    let kernel = KernelAccount::decode(&get(&mut scenario.bank, f.kernel).await.data).unwrap();
    assert_eq!(kernel.basis_mode, BasisMode::DerivedBasis);
    assert_eq!(kernel.total_supply[..4], [SETS; 4]);
    let supply =
        SupplyLedgerAccount::decode(&get(&mut scenario.bank, f.supply).await.data).unwrap();
    assert_eq!(supply.internal_supply[..4], [SETS; 4]);
    assert_eq!(supply.external_supply[..4], [0; 4]);
    assert_eq!(
        HoardAccount::decode(&get(&mut scenario.bank, f.hoard).await.data)
            .unwrap()
            .collateral_atoms,
        SETS
    );

    let materialize_units = succeed(
        &mut scenario.bank,
        &[
            budget(),
            f.materialize(actor.pubkey(), scenario.actor_outcome, 2, 0, external_lot),
        ],
        &[&actor],
    )
    .await;
    assert_eq!(
        scenario.token_amount(scenario.actor_outcome).await,
        external_lot
    );
    assert_eq!(scenario.mint_supply(f.outcome_mints[0]).await, external_lot);

    succeed(
        &mut scenario.bank,
        &[token_instruction::transfer_checked(
            &TOKEN_2022,
            &scenario.actor_outcome,
            &f.outcome_mints[0],
            &scenario.bearer_outcome,
            &actor.pubkey(),
            &[],
            external_lot,
            0,
        )
        .unwrap()],
        &[&actor],
    )
    .await;
    assert_eq!(scenario.token_amount(scenario.actor_outcome).await, 0);
    assert_eq!(
        scenario.token_amount(scenario.bearer_outcome).await,
        external_lot
    );

    let hostile_watch = [f.market, f.kernel, f.supply, f.resolution];
    let original_kernel =
        set_kernel_mode(&mut scenario.bank, f.kernel, BasisMode::FinitePreset).await;
    let hostile_before = snapshot(&mut scenario.bank, &hostile_watch).await;
    assert!(send(
        &mut scenario.bank,
        &[budget(), f.resolve(actor.pubkey(), f.source_archive)],
        &[&actor],
    )
    .await
    .0
    .is_err());
    assert_eq!(
        snapshot(&mut scenario.bank, &hostile_watch).await,
        hostile_before
    );
    scenario
        .bank
        .set_account(&f.kernel, &AccountSharedData::from(original_kernel));

    let before_substitution = snapshot(&mut scenario.bank, &hostile_watch).await;
    assert!(send(
        &mut scenario.bank,
        &[budget(), f.resolve(actor.pubkey(), SUBSTITUTE_ARCHIVE)],
        &[&actor],
    )
    .await
    .0
    .is_err());
    assert_eq!(
        snapshot(&mut scenario.bank, &hostile_watch).await,
        before_substitution
    );

    let resolve_units = succeed(
        &mut scenario.bank,
        &[budget(), f.resolve(actor.pubkey(), f.source_archive)],
        &[&actor],
    )
    .await;
    let resolution_bytes = get(&mut scenario.bank, f.resolution).await.data;
    assert_eq!(resolution_bytes.len(), NATIVE_RESOLUTION_LEN);
    let resolution = NativeResolutionAccount::decode(&resolution_bytes).unwrap();
    assert_eq!(resolution.mode, RESOLUTION_MODE_DERIVED_POINT);
    assert_eq!(resolution.payout_index, PAYOUT_INDEX_UNRESOLVED);
    assert_eq!(resolution.resolved_value, point);
    assert_eq!(resolution.vector.denominator, DENOMINATOR);
    assert_eq!(resolution.vector.weights, expected_weights);
    assert_eq!(resolution.window, f.window_id);
    let resolved_market =
        MarketAccount::decode(&get(&mut scenario.bank, f.market).await.data).unwrap();
    assert_eq!(resolved_market.lifecycle, 1);
    let resolved_kernel =
        KernelAccount::decode(&get(&mut scenario.bank, f.kernel).await.data).unwrap();
    assert_eq!(resolved_kernel.phase, 1);
    assert_eq!(resolved_kernel.basis_mode, BasisMode::DerivedBasis);

    let rollback_watch = [
        f.hoard,
        f.kernel,
        f.supply,
        f.resolution,
        f.hoard_token,
        scenario.bearer_outcome,
        f.outcome_mints[0],
        OVERFLOW_DESTINATION,
    ];
    let before_late_cpi = snapshot(&mut scenario.bank, &rollback_watch).await;
    assert!(send(
        &mut scenario.bank,
        &[
            budget(),
            f.redeem_external(
                bearer.pubkey(),
                scenario.bearer_outcome,
                OVERFLOW_DESTINATION,
                external_lot,
            ),
        ],
        &[&bearer],
    )
    .await
    .0
    .is_err());
    assert_eq!(
        snapshot(&mut scenario.bank, &rollback_watch).await,
        before_late_cpi
    );

    let external_units = succeed(
        &mut scenario.bank,
        &[
            budget(),
            f.redeem_external(
                bearer.pubkey(),
                scenario.bearer_outcome,
                scenario.bearer_collateral,
                external_lot,
            ),
        ],
        &[&bearer],
    )
    .await;
    let external_payout = expected_weights[0] * external_lot / DENOMINATOR;
    assert_eq!(external_payout, 1);
    assert_eq!(scenario.token_amount(scenario.bearer_outcome).await, 0);
    assert_eq!(scenario.mint_supply(f.outcome_mints[0]).await, 0);
    assert_eq!(
        scenario.token_amount(scenario.bearer_collateral).await,
        external_payout
    );
    assert_eq!(
        get(&mut scenario.bank, f.resolution).await.data,
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
            get(&mut scenario.bank, f.resolution).await.data,
            resolution_bytes
        );
    }
    assert_eq!(internal_payout + external_payout, SETS);

    let position =
        PositionAccount::decode(&get(&mut scenario.bank, f.position).await.data).unwrap();
    assert_eq!(position.internal[..4], [0; 4]);
    assert_eq!(position.cash_atoms, internal_payout);
    let replay = ReplayAccount::decode(&get(&mut scenario.bank, f.replay).await.data).unwrap();
    assert_eq!(replay.sequence, sequence);
    let supply =
        SupplyLedgerAccount::decode(&get(&mut scenario.bank, f.supply).await.data).unwrap();
    assert_eq!(supply.internal_supply[..4], [0; 4]);
    assert_eq!(supply.external_supply[..4], [0; 4]);
    let kernel = KernelAccount::decode(&get(&mut scenario.bank, f.kernel).await.data).unwrap();
    assert_eq!(kernel.total_supply[..4], [0; 4]);
    assert_eq!(
        HoardAccount::decode(&get(&mut scenario.bank, f.hoard).await.data)
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
        PositionAccount::decode(&get(&mut scenario.bank, f.position).await.data).unwrap();
    assert_eq!(position.cash_atoms, 0);
    assert_eq!(position.reserved_cash_atoms, 0);
    let replay = ReplayAccount::decode(&get(&mut scenario.bank, f.replay).await.data).unwrap();
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
        get(&mut scenario.bank, f.resolution).await.data,
        resolution_bytes
    );

    println!(
        "native full d{degree} point={point}: Endow {endow_units}, Split {split_units}, Materialize {materialize_units}, Resolve {resolve_units}, ExternalRedeem {external_units}, InternalRedeem {redeem_units:?}, Withdraw {withdraw_units} CU"
    );
}

#[tokio::test]
async fn blank_bank_native_degrees_one_through_three_reach_zero_hoard() {
    for degree in 1..=3 {
        run_degree(degree).await;
    }
}
