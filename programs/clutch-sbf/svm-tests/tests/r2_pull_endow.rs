//! The explicit mock-source ELF takes custody against a registered pull release.
//!
//! The production-default registry remains empty and refuses every fabricated
//! identity. This file exercises the same custody boundary in the distinct ELF
//! built with `non-production-mock-source`.
//!
//! This file is laboratory evidence for the shared runtime path:
//!
//! 1. a market whose immutable Terms bind a **SourceSpec v2** feed identity,
//!    with the 404-byte spec account installed at genesis, is endowed
//!    successfully — real Token-2022 collateral moves from the owner's account
//!    into the Hoard;
//! 2. the same plane with a spec naming a release this ELF does not carry still
//!    refuses `0x79`, byte-identically; and
//! 3. the original V1 plane — the one `prefund_creation.rs` asserts against —
//!    still refuses `0x79` too.
//!
//! Together those are the property the promotion plan requires of any registry
//! flip: **the refusal boundary narrows, it never disappears.**
//!
//! ## What this is not
//!
//! The spec's identity is `clutch_sbf::source_identity::fixture`, whose
//! receiver program is a program-derived address that no party can deploy to
//! (`r2_pull_identity.rs` checks this). So this is laboratory evidence that the
//! *runtime path* is correct. It is not production-provider evidence and it
//! pins no production byte: `source_identity::mainnet` is still entirely empty.
//!
//! There is also no public construction route for a v2 spec account yet — its
//! intent needs a layout-crate tag — so the account is installed at genesis,
//! exactly as the existing default campaign installs a canonical V1 image.

use {
    clutch_sbf::{
        instructions::genesis,
        seeds,
        source_archive_v2::{initialize_source_spec_v2_account, SOURCE_SPEC_ACCOUNT_V2_BYTES},
        source_identity::fixture,
        source_v2::spec::{SourceSpecFieldsV2, SourceSpecV2},
    },
    clutch_solana_layout::{account_len, Hash32, Intent, MarketAccount},
    clutch_svm_fixture::{
        build_plane, compute_unit_limit_data, fixture_terms, immutable_owner_account_bytes,
        layout_request, GenesisAccount, Mode, Plane, CASH_ATOMS, COMPUTE_BUDGET, FUNDED_SETS,
        MARKET_NONCE, PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM, TOKEN_2022,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_pack::Pack,
    solana_program_test::{tokio, BanksClient, ProgramTest},
    solana_signer::Signer,
    solana_system_interface::{instruction as system_instruction, program as system_program},
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
    spl_token_2022_interface::{
        instruction as token_instruction,
        instruction::AuthorityType,
        state::{Account as TokenAccount, Mint},
    },
};

const CU_LIMIT: u32 = 1_400_000;
const COLLATERAL_DECIMALS: u8 = 6;
const CREATOR_LAMPORTS: u64 = 10_000_000_000;
const OWNER_TOKENS: u64 = 5_000;
const DEPOSIT: u64 = 1_500;

/* ------------------------------------------------------------------------ */
/* The v2 spec this plane binds                                              */
/* ------------------------------------------------------------------------ */

/// A pull spec naming the compiled fixture release.
///
/// The grid triple deliberately equals the one `fixture_terms` writes (family
/// 7, version 1, 60-second buckets). The v2 Terms binding does not cross-check
/// the grid — Terms owns the observation window and the spec owns the source —
/// but a spec that disagreed with the market's own grid would be an incoherent
/// fixture even where nothing rejects it.
fn pull_spec_fields() -> SourceSpecFieldsV2 {
    fixture::REGISTERED_SPEC_FIELDS
}

fn registered_spec() -> SourceSpecV2 {
    SourceSpecV2::new(pull_spec_fields()).expect("the fixture pull spec is valid")
}

/// A structurally valid spec naming a parser release this ELF does not carry.
fn unregistered_spec() -> SourceSpecV2 {
    let mut fields = pull_spec_fields();
    fields.parser_version += 1;
    SourceSpecV2::new(fields).expect("still a valid v2 spec")
}

/* ------------------------------------------------------------------------ */
/* Repointing a fixture plane at a v2 source generation                      */
/* ------------------------------------------------------------------------ */

fn pda(seeds: &[&[u8]]) -> (Address, u8) {
    Address::find_program_address(seeds, &PROGRAM_ID)
}

fn encode<F>(len: usize, writer: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, clutch_solana_layout::CodecError>,
{
    let mut bytes = vec![0; len];
    assert_eq!(writer(&mut bytes).unwrap(), len);
    bytes
}

/// Rebuild a fixture plane's Terms, Market, and SourceSpec around one v2 spec.
///
/// Every other account — Realm, Profile, policy, Hoard, supply, the token
/// plane — is untouched, because none of them names a feed. What changes is
/// exactly what a different source generation changes: the feed identity, the
/// Terms digest that carries it, the Market body that binds both, and the spec
/// account itself.
fn repoint_to_pull_v2(plane: &mut Plane, spec: SourceSpecV2) {
    let feed_id = Hash32::from_bytes(spec.feed_id());
    let realm_seed = plane.realm_id.bytes();

    /* Terms is self-certifying: its digest is over its body and its address is
     * terms_pda(realm, digest), with the stored bump outside the body.  So the
     * digest is computed first and the canonical bump written after. */
    let mut terms = fixture_terms(plane.realm_id, plane.profile_id, feed_id);
    terms.source_adapter_id = Hash32::from_bytes(fixture::SOURCE_ADAPTER_ID);
    terms.source_version = fixture::SOURCE_ADAPTER_VERSION;
    terms.terms = terms
        .recomputed_terms_digest()
        .expect("the repointed terms body encodes");
    let terms_id = terms.terms;
    let (terms_address, terms_bump) = pda(&[seeds::SEED_TERMS, &realm_seed, &terms_id.bytes()]);
    terms.stored_bump = terms_bump;
    assert_eq!(
        terms
            .recomputed_terms_digest()
            .expect("the repointed terms body encodes"),
        terms_id,
        "the stored bump must be outside the digest body"
    );

    let (spec_address, spec_bump) = pda(&[seeds::SEED_SOURCE_SPEC, &feed_id.bytes()]);
    let mut spec_image = vec![0_u8; SOURCE_SPEC_ACCOUNT_V2_BYTES];
    initialize_source_spec_v2_account(&mut spec_image, spec, spec_bump)
        .expect("the v2 spec account image encodes");

    /* The Market body names both the Terms digest and the feed identity, and
     * `Endow` compares the presented Terms and SourceSpec against exactly
     * those two fields.  Its address does not depend on either, so the market
     * account is rewritten in place rather than moved. */
    let market_entry = plane
        .accounts
        .iter_mut()
        .find(|item| item.address == plane.market.address)
        .expect("the funded plane installs a market account");
    let mut market = MarketAccount::decode(&market_entry.data).expect("the market image decodes");
    market.terms = terms_id;
    market.feed = feed_id;
    market_entry.data = encode(account_len::MARKET, |out| market.encode(out));

    plane.accounts.push(GenesisAccount {
        address: terms_address,
        owner: PROGRAM_ID,
        data: encode(account_len::TERMS, |out| terms.encode(out)),
    });
    plane.accounts.push(GenesisAccount {
        address: spec_address,
        owner: PROGRAM_ID,
        data: spec_image,
    });

    plane.terms_id = terms_id;
    plane.terms.address = terms_address;
    plane.terms.bump = terms_bump;
    plane.source_spec.address = spec_address;
    plane.source_spec.bump = spec_bump;
    plane.feed_id = feed_id;
}

/* ------------------------------------------------------------------------ */
/* Harness                                                                   */
/* ------------------------------------------------------------------------ */

fn creator_keypair() -> Keypair {
    Keypair::new_from_array([
        0x93, 0x1a, 0x72, 0x4e, 0x05, 0xbc, 0x61, 0x38, 0xdf, 0x20, 0xa4, 0x59, 0x13, 0xe7, 0x8c,
        0x42, 0x6d, 0x99, 0x31, 0x0f, 0xa8, 0x55, 0xc2, 0x17, 0x4b, 0xe0, 0x76, 0x2a, 0x8d, 0x44,
        0xf3, 0x6c,
    ])
}

fn owner_keypair() -> Keypair {
    Keypair::new_from_array([
        0x71, 0x08, 0xd4, 0x39, 0xb6, 0x2f, 0x83, 0x15, 0xca, 0x64, 0x20, 0x9e, 0x47, 0xf1, 0x5b,
        0x32, 0xad, 0x06, 0x78, 0xc3, 0x11, 0xe5, 0x59, 0x24, 0x8a, 0x4d, 0x90, 0x37, 0xfb, 0x6e,
        0x02, 0xac,
    ])
}

fn collateral_mint_keypair() -> Keypair {
    Keypair::new_from_array([
        0x2c, 0xe1, 0x17, 0x95, 0x68, 0x34, 0xab, 0x40, 0x73, 0x0d, 0xf9, 0x52, 0x86, 0x1b, 0xc5,
        0x29, 0x74, 0xae, 0x03, 0x67, 0x9c, 0x41, 0xd8, 0x10, 0x5a, 0xb3, 0x26, 0xee, 0x80, 0x4f,
        0x19, 0x62,
    ])
}

fn rent_exempt(space: usize) -> u64 {
    solana_rent::Rent::default().minimum_balance(space).max(1)
}

fn system_slot(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn budget() -> Instruction {
    Instruction::new_with_bytes(
        COMPUTE_BUDGET,
        &compute_unit_limit_data(CU_LIMIT),
        Vec::new(),
    )
}

async fn try_send(
    banks: &mut BanksClient,
    payer: &Keypair,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), TransactionError> {
    let blockhash = banks.get_latest_blockhash().await.unwrap();
    let mut all = vec![payer];
    all.extend_from_slice(signers);
    let transaction =
        Transaction::new_signed_with_payer(instructions, Some(&payer.pubkey()), &all, blockhash);
    banks
        .process_transaction(transaction)
        .await
        .map_err(|error| match error {
            solana_program_test::BanksClientError::TransactionError(inner) => inner,
            solana_program_test::BanksClientError::SimulationError { err, .. } => err,
            other => panic!("unexpected banks error: {other:?}"),
        })
}

async fn send(
    banks: &mut BanksClient,
    payer: &Keypair,
    instructions: &[Instruction],
    signers: &[&Keypair],
) {
    try_send(banks, payer, instructions, signers)
        .await
        .expect("transaction was expected to succeed");
}

async fn token_amount(banks: &mut BanksClient, address: Address) -> u64 {
    let account = banks
        .get_account(address)
        .await
        .unwrap()
        .expect("token account must exist");
    TokenAccount::unpack(&account.data[..TokenAccount::LEN])
        .expect("token account decodes")
        .amount
}

async fn create_collateral_mint(banks: &mut BanksClient, payer: &Keypair, mint: &Keypair) {
    let lamports = rent_exempt(Mint::LEN);
    send(
        banks,
        payer,
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &mint.pubkey(),
                lamports,
                Mint::LEN as u64,
                &TOKEN_2022,
            ),
            token_instruction::initialize_mint2(
                &TOKEN_2022,
                &mint.pubkey(),
                &payer.pubkey(),
                None,
                COLLATERAL_DECIMALS,
            )
            .unwrap(),
        ],
        &[mint],
    )
    .await;
}

async fn create_token_account(
    banks: &mut BanksClient,
    payer: &Keypair,
    mint: Address,
    owner: Address,
) -> Address {
    let token = Keypair::new();
    let lamports = rent_exempt(TokenAccount::LEN);
    send(
        banks,
        payer,
        &[
            system_instruction::create_account(
                &payer.pubkey(),
                &token.pubkey(),
                lamports,
                TokenAccount::LEN as u64,
                &TOKEN_2022,
            ),
            token_instruction::initialize_account3(&TOKEN_2022, &token.pubkey(), &mint, &owner)
                .unwrap(),
        ],
        &[&token],
    )
    .await;
    token.pubkey()
}

/// Which source generation a world's market binds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Source {
    /// The stock V1 fixture plane, whose release this ELF does not carry.
    LegacyV1,
    /// A pull spec naming the compiled fixture release.
    RegisteredPullV2,
    /// A pull spec naming a release this ELF does not carry.
    UnregisteredPullV2,
}

struct World {
    banks: BanksClient,
    payer: Keypair,
    owner: Keypair,
    plane: Plane,
    owner_token: Address,
    position: Address,
    replay: Address,
}

impl World {
    async fn start(source: Source) -> Self {
        let founder = creator_keypair();
        let owner = owner_keypair();
        let mint = collateral_mint_keypair();
        let mut plane = build_plane(founder.pubkey(), mint.pubkey(), MARKET_NONCE, Mode::Funded);
        match source {
            Source::LegacyV1 => {}
            Source::RegisteredPullV2 => repoint_to_pull_v2(&mut plane, registered_spec()),
            Source::UnregisteredPullV2 => repoint_to_pull_v2(&mut plane, unregistered_spec()),
        }
        let (position, replay) = plane.owner_plane(owner.pubkey());

        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.add_program("clutch_sbf", PROGRAM_ID, None);
        test.add_account(founder.pubkey(), system_slot(CREATOR_LAMPORTS));
        test.add_account(owner.pubkey(), system_slot(CREATOR_LAMPORTS));
        for item in &plane.accounts {
            test.add_account(
                item.address,
                Account {
                    lamports: rent_exempt(item.data.len()),
                    data: item.data.clone(),
                    owner: item.owner,
                    executable: false,
                    rent_epoch: 0,
                },
            );
        }
        let hoard_data = immutable_owner_account_bytes(
            mint.pubkey(),
            plane.hoard_authority.address,
            FUNDED_SETS + CASH_ATOMS,
        );
        test.add_account(
            plane.hoard_token.address,
            Account {
                lamports: rent_exempt(hoard_data.len()),
                data: hoard_data,
                owner: TOKEN_2022,
                executable: false,
                rent_epoch: 0,
            },
        );

        let (mut banks, payer, _) = test.start().await;
        create_collateral_mint(&mut banks, &payer, &mint).await;
        let owner_token =
            create_token_account(&mut banks, &payer, mint.pubkey(), owner.pubkey()).await;
        send(
            &mut banks,
            &payer,
            &[token_instruction::mint_to(
                &TOKEN_2022,
                &mint.pubkey(),
                &owner_token,
                &payer.pubkey(),
                &[],
                OWNER_TOKENS,
            )
            .unwrap()],
            &[],
        )
        .await;
        send(
            &mut banks,
            &payer,
            &[token_instruction::set_authority(
                &TOKEN_2022,
                &mint.pubkey(),
                None,
                AuthorityType::MintTokens,
                &payer.pubkey(),
                &[],
            )
            .unwrap()],
            &[],
        )
        .await;

        Self {
            banks,
            payer,
            owner,
            plane,
            owner_token,
            position: position.address,
            replay: replay.address,
        }
    }

    fn endow(&self, amount: u64) -> Instruction {
        let metas = vec![
            AccountMeta::new(self.owner.pubkey(), true),
            AccountMeta::new_readonly(self.plane.market.address, false),
            AccountMeta::new_readonly(self.plane.hoard.address, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.replay, false),
            AccountMeta::new_readonly(self.plane.profile.address, false),
            AccountMeta::new_readonly(self.plane.policy_account, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.plane.collateral_mint, false),
            AccountMeta::new(self.owner_token, false),
            AccountMeta::new(self.plane.hoard_token.address, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(self.plane.terms.address, false),
            AccountMeta::new_readonly(self.plane.source_spec.address, false),
        ];
        assert_eq!(metas.len(), genesis::ENDOW_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            PROGRAM_ID,
            &layout_request(
                0,
                Intent::Endow {
                    market: self.plane.market_id,
                    owner: Hash32::from_bytes(self.owner.pubkey().to_bytes()),
                    amount,
                },
            ),
            metas,
        )
    }
}

fn source_release_unavailable() -> TransactionError {
    TransactionError::InstructionError(
        1,
        InstructionError::Custom(clutch_sbf::error::ClutchError::SourceReleaseUnavailable as u32),
    )
}

/* ------------------------------------------------------------------------ */
/* The evidence                                                              */
/* ------------------------------------------------------------------------ */

#[cfg(feature = "non-production-mock-source")]
#[tokio::test]
async fn the_mock_elf_takes_custody_against_its_registered_pull_release() {
    let mut world = World::start(Source::RegisteredPullV2).await;
    let payer = world.payer.insecure_clone();
    let owner = world.owner.insecure_clone();

    // The 404-byte v2 spec account really is what the market binds.
    let spec_account = world
        .banks
        .get_account(world.plane.source_spec.address)
        .await
        .unwrap()
        .expect("the v2 spec account is installed");
    assert_eq!(spec_account.data.len(), SOURCE_SPEC_ACCOUNT_V2_BYTES);
    assert_eq!(spec_account.owner, PROGRAM_ID);

    let hoard_before = token_amount(&mut world.banks, world.plane.hoard_token.address).await;
    let owner_before = token_amount(&mut world.banks, world.owner_token).await;
    assert_eq!(owner_before, OWNER_TOKENS);
    assert!(world
        .banks
        .get_account(world.position)
        .await
        .unwrap()
        .is_none());

    let instruction = world.endow(DEPOSIT);
    send(
        &mut world.banks,
        &payer,
        &[budget(), instruction],
        &[&owner],
    )
    .await;

    /* Real Token-2022 collateral moved, and the owner plane was constructed
     * inside the explicitly non-production mock-source ELF. */
    assert_eq!(
        token_amount(&mut world.banks, world.owner_token).await,
        owner_before - DEPOSIT
    );
    assert_eq!(
        token_amount(&mut world.banks, world.plane.hoard_token.address).await,
        hoard_before + DEPOSIT
    );
    let position = world
        .banks
        .get_account(world.position)
        .await
        .unwrap()
        .expect("the owner plane was constructed");
    assert_eq!(position.owner, PROGRAM_ID);
    assert_eq!(position.data.len(), account_len::POSITION);
}

#[cfg(not(any(
    feature = "non-production-mock-source",
    feature = "non-production-real-pyth-lab"
)))]
#[tokio::test]
async fn the_default_elf_refuses_the_fixture_release_and_writes_nothing() {
    let mut world = World::start(Source::RegisteredPullV2).await;
    let payer = world.payer.insecure_clone();
    let owner = world.owner.insecure_clone();
    let owner_before = token_amount(&mut world.banks, world.owner_token).await;
    let hoard_before = token_amount(&mut world.banks, world.plane.hoard_token.address).await;
    let instruction = world.endow(DEPOSIT);

    assert_eq!(
        try_send(
            &mut world.banks,
            &payer,
            &[budget(), instruction],
            &[&owner]
        )
        .await,
        Err(source_release_unavailable())
    );
    assert_eq!(token_amount(&mut world.banks, world.owner_token).await, owner_before);
    assert_eq!(
        token_amount(&mut world.banks, world.plane.hoard_token.address).await,
        hoard_before
    );
    assert!(world.banks.get_account(world.position).await.unwrap().is_none());
}

#[cfg(not(feature = "non-production-mock-source"))]
#[tokio::test]
async fn an_unregistered_pull_release_still_refuses_and_writes_nothing() {
    let mut world = World::start(Source::UnregisteredPullV2).await;
    let payer = world.payer.insecure_clone();
    let owner = world.owner.insecure_clone();

    // The spec is structurally perfect and correctly bound to its Terms; the
    // only thing wrong with it is that this ELF carries no such release.
    let spec_account = world
        .banks
        .get_account(world.plane.source_spec.address)
        .await
        .unwrap()
        .expect("the v2 spec account is installed");
    assert_eq!(spec_account.data.len(), SOURCE_SPEC_ACCOUNT_V2_BYTES);

    let owner_before = token_amount(&mut world.banks, world.owner_token).await;
    let hoard_before = token_amount(&mut world.banks, world.plane.hoard_token.address).await;

    let instruction = world.endow(DEPOSIT);
    assert_eq!(
        try_send(
            &mut world.banks,
            &payer,
            &[budget(), instruction],
            &[&owner]
        )
        .await,
        Err(source_release_unavailable())
    );

    assert_eq!(
        token_amount(&mut world.banks, world.owner_token).await,
        owner_before
    );
    assert_eq!(
        token_amount(&mut world.banks, world.plane.hoard_token.address).await,
        hoard_before
    );
    assert!(world
        .banks
        .get_account(world.position)
        .await
        .unwrap()
        .is_none());
}

#[cfg(not(feature = "non-production-mock-source"))]
#[tokio::test]
async fn the_v1_registry_is_still_empty_on_the_same_harness() {
    // The narrowing is generation-local: admitting one v2 release did not
    // admit anything under V1, whose compiled registry is still hard-`false`.
    let mut world = World::start(Source::LegacyV1).await;
    let payer = world.payer.insecure_clone();
    let owner = world.owner.insecure_clone();
    let instruction = world.endow(DEPOSIT);
    assert_eq!(
        try_send(
            &mut world.banks,
            &payer,
            &[budget(), instruction],
            &[&owner]
        )
        .await,
        Err(source_release_unavailable())
    );
}

#[cfg(not(feature = "non-production-mock-source"))]
#[tokio::test]
async fn the_two_generations_derive_different_feeds_terms_and_spec_addresses() {
    // A structural check that the repointing is a real generation change
    // rather than a relabelling: the v2 domain separation moves the feed
    // identity, which moves the Terms digest and every feed-derived address.
    let founder = creator_keypair();
    let mint = collateral_mint_keypair();
    let v1 = build_plane(founder.pubkey(), mint.pubkey(), MARKET_NONCE, Mode::Funded);
    let mut v2 = build_plane(founder.pubkey(), mint.pubkey(), MARKET_NONCE, Mode::Funded);
    repoint_to_pull_v2(&mut v2, registered_spec());

    assert_ne!(v1.feed_id, v2.feed_id);
    assert_ne!(v1.terms_id, v2.terms_id);
    assert_ne!(v1.terms.address, v2.terms.address);
    assert_ne!(v1.source_spec.address, v2.source_spec.address);
    // The market keeps its identity and address: a market is not its feed.
    assert_eq!(v1.market_id, v2.market_id);
    assert_eq!(v1.market.address, v2.market.address);
}
