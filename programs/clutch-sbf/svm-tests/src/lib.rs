//! The genesis plane the SVM scenarios drive, built from the frozen encoders.
//!
//! Every account this module produces is encoded by the *same*
//! `clutch-solana-layout` and `clutch-solana-reference` codecs the program
//! decodes with, and every address is derived from the *same*
//! `clutch_sbf::seeds` prefixes the program derives with. Nothing here is a
//! second description of the protocol: a divergence would be a divergence
//! inside one crate, not between two hand-kept copies.
//!
//! Off-chain program-address derivation is the one thing that cannot be shared.
//! `clutch_sbf::seeds::find` is `unimplemented!()` for the host on purpose —
//! `programs/clutch-sbf` cannot enable `solana-pubkey`'s `curve25519` backend
//! without breaking `cargo-build-sbf`'s offline `cargo metadata` — so this
//! workspace derives with `solana_address::Address::find_program_address` over
//! the exported seed constants. The seed *bytes* stay single-sourced; only the
//! derivation call differs, and the SVM is what proves the two agree: a
//! mismatch is `ClutchError::WrongPda` on the first transaction.
//!
//! ## What is loaded at genesis, and why
//!
//! Pre-funded scenarios install their historical state at genesis. The empty
//! market scenario deliberately does not: the canonical Market, Hoard,
//! founding Position, kernel, Replay, SupplyLedger, Resolution, outcome mints,
//! and Hoard token account are absent until `CreateMarket` signs the required
//! System-program CPIs.
//!
//! For the outcome mints that is a real claim to defend: these are 82 bytes
//! this crate wrote, not bytes Token-2022 wrote. They are not taken on trust —
//! the real Token-2022 program is what executes `MintTo` and `Burn` against
//! them through the program's CPI, and it refuses anything it did not consider
//! a mint. A wrong byte here is a failing test, not a passing one.

#![deny(missing_docs)]

use clutch_kernel::{BasisMode, PayoutSet, PayoutVector};
use clutch_sbf::instructions::observe_resolve::{
    BUFFER_VERSION, EVIDENCE_BUFFER_HEADER_BYTES, EVIDENCE_BUFFER_TAG,
};
use clutch_sbf::seeds;
use clutch_sbf::source::{
    ParsedPriceV1, PriceParserV1, SourceAccountView, SourceError, SourceSpecFieldsV1, SourceSpecV1,
    TrustedClockV1, ORIENTATION_QUOTE_PER_BASE, SELECTION_FINALIZED_BUCKET_RECORD,
};
use clutch_sbf::source_archive::{
    append_authenticated, canonical_window_id, initialize_archive, initialize_source_spec_account,
    seal_archive, verify_source_spec_account, ArchivePredecessorV1, CoveragePolicy,
    DeploymentAuthenticatorV1, FeedIdentity, Grid, RuntimeAccountViewV1, SourceArchiveError,
    SourceSpecAccountViewV1, WindowDomain, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES,
    SOURCE_ARCHIVE_MAX_RECORDS_V1, SOURCE_SPEC_ACCOUNT_V1_BYTES,
};
use clutch_solana_layout::{
    account_len, canonical_market_id, canonical_outcome_id, canonical_realm_id, collateral,
    FeedAccount, FeedId, Hash32, HoardAccount, Intent, MarketAccount, PayoutVectorBytes,
    PositionAccount, ProfileAccount, RealmAccount, ResolutionAccount, SupplyLedgerAccount,
    TermsAccount, MAX_INTENT_BYTES, MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_INDEX_UNRESOLVED,
    PAYOUT_MAP_UNUSED, PROFILE_FLAG_POLICY_FROZEN, UNIFORM_SPACING_NONE,
};
use clutch_solana_reference::{
    ExternalAccount, KernelAccount, ReplayAccount, EXTERNAL_ACCOUNT_LEN, KERNEL_ACCOUNT_LEN,
    REPLAY_ACCOUNT_LEN,
};
use solana_address::Address;

/// The address the program ELF is deployed at in these tests.
///
/// Arbitrary: an SBF program is position independent and its id is whatever it
/// is deployed at. It is fixed rather than random so that every derived address
/// in a recorded run is reproducible.
pub const PROGRAM_ID: Address = Address::new_from_array([
    0x0c, 0x1a, 0x7c, 0x48, 0x2d, 0x1f, 0x93, 0x55, 0x0b, 0x6e, 0x2a, 0x11, 0x77, 0x40, 0xd8, 0x39,
    0x51, 0x88, 0x3c, 0x2b, 0x64, 0xa0, 0xfe, 0x17, 0x05, 0xcc, 0x91, 0x3d, 0x22, 0x6b, 0x84, 0x70,
]);

/// The Token-2022 program id, taken from the frozen layout crate.
pub const TOKEN_2022: Address = Address::new_from_array(collateral::TOKEN_2022_PROGRAM);

/// Fixture-only reviewed-adapter identity. It is not a production source.
pub const SOURCE_ADAPTER_ID: [u8; 32] = [0xa1; 32];
const SOURCE_PROGRAM: [u8; 32] = [0xb2; 32];
const SOURCE_PROGRAM_OWNER: [u8; 32] = [0xb3; 32];
const SOURCE_DATA_ACCOUNT: [u8; 32] = [0xc3; 32];
const SOURCE_DEPLOYMENT_ACCOUNT: [u8; 32] = [0xd4; 32];
const SOURCE_DEPLOYMENT_OWNER: [u8; 32] = [0xd5; 32];
const SOURCE_DEPLOYMENT_VERIFIER: [u8; 32] = [0xd6; 32];
const SOURCE_DEPLOYMENT_GENERATION: u64 = 19;
const SOURCE_RECORD_BYTES: usize = 77;

struct FixtureDeployment;

impl DeploymentAuthenticatorV1 for FixtureDeployment {
    const VERIFIER_ID: [u8; 32] = SOURCE_DEPLOYMENT_VERIFIER;
    const VERIFIER_VERSION: u32 = 1;
    const PROVIDER_PROGRAM: [u8; 32] = SOURCE_PROGRAM;
    const PROVIDER_PROGRAM_OWNER: [u8; 32] = SOURCE_PROGRAM_OWNER;
    const DEPLOYMENT_ACCOUNT: [u8; 32] = SOURCE_DEPLOYMENT_ACCOUNT;
    const DEPLOYMENT_OWNER: [u8; 32] = SOURCE_DEPLOYMENT_OWNER;

    fn deployment_generation(
        provider_program_data: &[u8],
        deployment_account_data: &[u8],
    ) -> Result<u64, SourceArchiveError> {
        if provider_program_data != b"fixture-provider-v1"
            || deployment_account_data != b"fixture-deployment-v1"
        {
            return Err(SourceArchiveError::DeploymentAdapterRefused);
        }
        Ok(SOURCE_DEPLOYMENT_GENERATION)
    }
}

struct FixturePriceParser;

impl PriceParserV1 for FixturePriceParser {
    const SOURCE_ADAPTER_ID: [u8; 32] = SOURCE_ADAPTER_ID;
    const SOURCE_ADAPTER_VERSION: u32 = 7;
    const PARSER_ID: u16 = 11;
    const PARSER_VERSION: u16 = 3;

    fn parse(account: SourceAccountView<'_>) -> Result<ParsedPriceV1, SourceError> {
        let bytes = account.data();
        if bytes.len() != SOURCE_RECORD_BYTES || &bytes[..4] != b"SRC1" {
            return Err(SourceError::ParserRefused);
        }
        let u64_at = |offset: usize| {
            let mut value = [0_u8; 8];
            value.copy_from_slice(&bytes[offset..offset + 8]);
            u64::from_le_bytes(value)
        };
        let u128_at = |offset: usize| {
            let mut value = [0_u8; 16];
            value.copy_from_slice(&bytes[offset..offset + 16]);
            u128::from_le_bytes(value)
        };
        Ok(ParsedPriceV1 {
            deployment_generation: u64_at(4),
            source_sequence: u64_at(12),
            publish_slot: u64_at(20),
            publish_time: u64_at(28),
            canonical_bucket: u64_at(36),
            price_atoms: u128_at(44),
            confidence_atoms: u128_at(60),
            finalized_bucket: bytes[76] == 1,
        })
    }
}

/// Realm nonce, and the market nonces of the two fixture markets.
const REALM_NONCE: u64 = 7;
/// The pre-funded market the seam and redemption scenarios drive.
pub const MARKET_NONCE: u64 = 9;
/// The market `CreateMarket` founds from nothing.
pub const FOUNDING_MARKET_NONCE: u64 = 11;
/// Active outcomes; the payout set has one vector per outcome.
pub const OUTCOME_COUNT: u8 = 2;
/// Position generation the founding triple lives at.
pub const GENERATION: u64 = 0;
/// Complete sets the fixture is pre-funded with.
pub const FUNDED_SETS: u64 = 20;
/// Free cash the founding position keeps after that funding.
pub const CASH_ATOMS: u64 = 80;
/// Encumbered cash, untouched by every seam transition.
pub const RESERVED_CASH_ATOMS: u64 = 7;
/// Immutable collateral ceiling of the fixture market.
pub const COLLATERAL_CAP: u64 = 1_000;

/// The three bytes of the reference request envelope this harness re-emits.
///
/// They are private constants of `clutch_solana_reference`. [`layout_request`]
/// keeps this copy honest by refusing to return bytes that do not decode back
/// through the real `Request::decode`.
const REQUEST_TAG: u8 = 0xd1;
const REFERENCE_VERSION: u8 = 1;
const ACTION_LAYOUT: u8 = 0;

/// A derived address and its canonical bump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Pda {
    /// The address.
    pub address: Address,
    /// The canonical bump seed.
    pub bump: u8,
}

fn derive(seeds: &[&[u8]]) -> Pda {
    let (address, bump) = Address::find_program_address(seeds, &PROGRAM_ID);
    Pda { address, bump }
}

/// One account to install at genesis.
#[derive(Clone, Debug)]
pub struct GenesisAccount {
    /// Where it goes.
    pub address: Address,
    /// Which program owns it.
    pub owner: Address,
    /// Its bytes.
    pub data: Vec<u8>,
}

/// Every address and every account image of one fixture market.
#[derive(Clone, Debug)]
pub struct Plane {
    /// Canonical Realm identity.
    pub realm_id: Hash32,
    /// Canonical Profile identity.
    pub profile_id: Hash32,
    /// Canonical market identity.
    pub market_id: Hash32,
    /// The actor, who owns the founding position and signs every seam request.
    pub actor: Address,
    /// Realm account.
    pub realm: Pda,
    /// Profile account.
    pub profile: Pda,
    /// Market account, and the outcome mints' authority.
    pub market: Pda,
    /// Hoard collateral-accounting account.
    pub hoard: Pda,
    /// Founding position account.
    pub position: Pda,
    /// Reference-only kernel aggregate.
    pub kernel: Pda,
    /// Reference-only external shadow.
    pub external: Pda,
    /// Reference-only replay sequence.
    pub replay: Pda,
    /// Market-wide two-term supply ledger.
    pub supply: Pda,
    /// The Hoard's signing authority, which holds no data.
    pub hoard_authority: Pda,
    /// The Hoard's Token-2022 account.
    pub hoard_token: Pda,
    /// One outcome mint per active outcome.
    pub outcome_mints: Vec<Pda>,
    /// The Realm's frozen collateral policy.
    pub policy: collateral::CollateralPolicy,
    /// Canonical sealed policy PDA, derived from Profile and policy digest.
    pub policy_account: Address,
    /// The collateral mint the policy names.
    pub collateral_mint: Address,
    /// Immutable terms artifact this market binds.
    pub terms: Pda,
    /// Its digest, which is `MarketAccount::terms`.
    pub terms_id: Hash32,
    /// Resolution record.
    pub resolution: Pda,
    /// Feed head.
    pub feed: Pda,
    /// The feed identity the market resolves against.
    pub feed_id: FeedId,
    /// Canonical immutable source-spec account.
    pub source_spec: Pda,
    /// Canonical sealed source archive for this market's exact window.
    pub source_archive: Pda,
    /// Evidence buffer used by the fixture.
    ///
    /// An active plane carries the complete sealed window needed by `Resolve`.
    /// A synthetically pre-resolved plane may carry only the identity header
    /// because `RedeemInternal` does not re-evaluate the window.  A real exact
    /// `Resolve` retry still presents the original complete sealed evidence.
    pub buffer: Address,
    /// The window identity the resolution record is sealed to.
    pub window_id: Hash32,
    /// Collateral atoms the Hoard holds at genesis.
    pub hoard_atoms: u64,
    /// The program-owned account images this plane installs at genesis.
    pub accounts: Vec<GenesisAccount>,
}

/// What state a fixture plane is loaded in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// Every market-specific target is absent from genesis. Existing
    /// Realm/Profile/Terms/policy/feed evidence remains fixture input.
    Empty,
    /// Pre-funded with [`FUNDED_SETS`] complete sets and active.
    Funded,
    /// The same, resolved to one payout index, so a redemption can run.
    Resolved(u8),
}

fn payout_set() -> PayoutSet {
    let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
    let mut left = [0; MAX_OUTCOMES];
    left[0] = 1;
    vectors[0] = PayoutVector::new(1, left);
    let mut right = [0; MAX_OUTCOMES];
    right[1] = 1;
    vectors[1] = PayoutVector::new(1, right);
    PayoutSet::new(2, OUTCOME_COUNT, vectors)
}

fn fixture_source_spec() -> SourceSpecV1 {
    SourceSpecV1::new(SourceSpecFieldsV1 {
        source_adapter_id: Hash32::from_bytes(SOURCE_ADAPTER_ID),
        source_adapter_version: 7,
        parser_id: 11,
        parser_version: 3,
        source_program: SOURCE_PROGRAM,
        source_account: SOURCE_DATA_ACCOUNT,
        deployment_generation: SOURCE_DEPLOYMENT_GENERATION,
        base_asset_id: Hash32::from_bytes([0xb1; 32]),
        quote_asset_id: Hash32::from_bytes([0xb2; 32]),
        orientation: ORIENTATION_QUOTE_PER_BASE,
        normalized_decimals: 0,
        grid_family_id: 7,
        grid_version: 1,
        bucket_seconds: 60,
        max_staleness_slots: 20,
        max_staleness_seconds: 120,
        max_future_seconds: 2,
        max_confidence_atoms: 1,
        max_confidence_bps: 10_000,
        confidence_multiplier: 1,
        selection_rule: SELECTION_FINALIZED_BUCKET_RECORD,
    })
    .expect("fixture source spec")
}

fn fixture_source_record(
    bucket: u64,
    sequence: u64,
    slot: u64,
    price: u128,
    confidence: u128,
) -> [u8; 77] {
    let mut out = [0_u8; SOURCE_RECORD_BYTES];
    out[..4].copy_from_slice(b"SRC1");
    out[4..12].copy_from_slice(&SOURCE_DEPLOYMENT_GENERATION.to_le_bytes());
    out[12..20].copy_from_slice(&sequence.to_le_bytes());
    out[20..28].copy_from_slice(&slot.to_le_bytes());
    out[28..36].copy_from_slice(&(bucket * 60).to_le_bytes());
    out[36..44].copy_from_slice(&bucket.to_le_bytes());
    out[44..60].copy_from_slice(&price.to_le_bytes());
    out[60..76].copy_from_slice(&confidence.to_le_bytes());
    out[76] = 1;
    out
}

/// The Realm's frozen collateral policy: a real, decodable 266-byte policy.
///
/// The collateral mint it names is a **live** mint the real Token-2022 program
/// created — the whole collateral leg reads it, and the Profile identity every
/// address in the plane descends from is the parent hash over this policy's
/// digest, so a fixture whose policy did not name the mint that exists would
/// not merely fail a check, it would derive different addresses.
pub fn fixture_policy(collateral_mint: [u8; 32]) -> collateral::CollateralPolicy {
    let backing = collateral::CurrencyRef::spl(collateral::TOKEN_2022_PROGRAM, collateral_mint, 6);
    collateral::CollateralPolicy {
        schema_version: collateral::COLLATERAL_POLICY_SCHEMA,
        flags: collateral::COLLATERAL_POLICY_STRICT_FLAGS,
        collateral: backing,
        fee: collateral::CurrencyRef::NATIVE_SOL,
        liveness: collateral::CurrencyRef::NATIVE_SOL,
        max_supply_atoms: 1_000_000_000_000_000,
        allowed_mint_extensions: 0,
        required_mint_extensions: 0,
        allowed_account_extensions: collateral::EXTENSION_IMMUTABLE_OWNER,
        required_account_extensions: 0,
    }
}

/// Build the fixture plane, pre-funded with [`FUNDED_SETS`] complete sets.
///
/// The funding is written directly rather than reached by a `Split`
/// transaction, and the state it writes is exactly a post-`Split` state: the
/// position holds *n* of every outcome internally, the market-wide ledger's
/// internal term is *n*, the kernel aggregate's total supply is *n*, the Hoard
/// holds *n* collateral atoms, and the position paid *n* cash for them. The
/// CLO-DELTA-V1 obligations C1 and C2 hold over it, which the program re-checks
/// before it touches anything — so a wrong fixture is a refused transaction and
/// not a false pass.
pub fn build_plane(actor: Address, collateral_mint: Address, nonce: u64, mode: Mode) -> Plane {
    let policy = fixture_policy(collateral_mint.to_bytes());
    /* The Profile identity is the parent hash over the policy's own digest,
     * recomputed rather than chosen: `verify_profile_identity` refuses any
     * other pairing, which is what makes the 266 bytes bindable from an
     * arbitrary account. */
    let profile_id = collateral::ParentProfile::from_policy(&policy)
        .expect("the fixture policy composes a parent profile")
        .identity()
        .expect("the parent profile derives an identity");
    let policy_digest = policy.digest().expect("the fixture policy must digest");
    let policy_artifact = derive(&[
        seeds::SEED_POLICY,
        &profile_id.bytes(),
        &policy_digest.bytes(),
    ]);
    let realm_id = canonical_realm_id(profile_id, REALM_NONCE);
    let market_id = canonical_market_id(realm_id, profile_id, nonce);
    let owner = Hash32::from_bytes(actor.to_bytes());

    let realm_seed = realm_id.bytes();
    let market_seed = market_id.bytes();
    let owner_seed = owner.bytes();
    let generation_seed = GENERATION.to_le_bytes();

    let realm = derive(&[seeds::SEED_REALM, &realm_seed]);
    let profile = derive(&[seeds::SEED_PROFILE, &realm_seed, &profile_id.bytes()]);
    let market = derive(&[seeds::SEED_MARKET, &realm_seed, &market_seed]);
    let hoard = derive(&[seeds::SEED_HOARD, &market_seed]);
    let position = derive(&[seeds::SEED_POSITION, &market_seed, &owner_seed]);
    let kernel = derive(&[seeds::SEED_KERNEL, &market_seed]);
    let external = derive(&[
        seeds::SEED_EXTERNAL,
        &market_seed,
        &owner_seed,
        &generation_seed,
    ]);
    let replay = derive(&[
        seeds::SEED_REPLAY,
        &market_seed,
        &owner_seed,
        &generation_seed,
    ]);
    let supply = derive(&[seeds::SEED_SUPPLY, &market_seed]);
    let hoard_authority = derive(&[seeds::SEED_HOARD_AUTHORITY, &market_seed]);
    let hoard_token = derive(&[seeds::SEED_HOARD_TOKEN, &market_seed]);
    let outcome_mints: Vec<Pda> = (0..OUTCOME_COUNT)
        .map(|outcome| derive(&[seeds::SEED_OUTCOME_MINT, &market_seed, &[outcome]]))
        .collect();

    let mut internal = [0_u64; MAX_OUTCOMES];
    for slot in internal.iter_mut().take(usize::from(OUTCOME_COUNT)) {
        *slot = FUNDED_SETS;
    }

    let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
    for (index, slot) in outcomes
        .iter_mut()
        .enumerate()
        .take(usize::from(OUTCOME_COUNT))
    {
        *slot = canonical_outcome_id(market_id, index as u8);
    }

    let source_spec_value = fixture_source_spec();
    let feed_id = source_spec_value.feed_id();
    /* The terms artifact is self-certifying: its digest is over its body, and
     * its address is `terms_pda(realm, digest)`.  The stored bump is outside
     * the body (`TERMS_BODY_BYTES` excludes it), so the digest can be computed
     * first and the canonical bump written afterwards without moving it. */
    let mut terms_account = fixture_terms(realm_id, profile_id, feed_id);
    let terms_id = terms_account.terms;
    let terms = derive(&[seeds::SEED_TERMS, &realm_seed, &terms_id.bytes()]);
    terms_account.stored_bump = terms.bump;
    assert_eq!(
        terms_account
            .recomputed_terms_digest()
            .expect("the fixture terms body encodes"),
        terms_id,
        "the stored bump must be outside the digest body"
    );
    let resolution = derive(&[seeds::SEED_RESOLUTION, &market_seed]);
    let feed = derive(&[seeds::SEED_FEED, &feed_id.bytes()]);
    let source_spec = derive(&[seeds::SEED_SOURCE_SPEC, &feed_id.bytes()]);
    let feed_identity = FeedIdentity::new(SOURCE_ADAPTER_ID, feed_id.bytes(), 7, 1)
        .expect("fixture source identity");
    let window_domain = WindowDomain::new(
        feed_identity,
        Grid::new(7, 1, 60).expect("fixture grid"),
        START_BUCKET,
        END_BUCKET,
        START_BUCKET + 4,
        0,
        CoveragePolicy::COMPLETE_REQUIRED,
    )
    .expect("fixture window domain");
    let window_id = canonical_window_id(window_domain);
    let source_archive = derive(&[
        seeds::SEED_SOURCE_ARCHIVE,
        &feed_id.bytes(),
        &window_id.bytes(),
    ]);
    let mut source_spec_bytes = vec![0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    initialize_source_spec_account(&mut source_spec_bytes, source_spec_value, source_spec.bump)
        .expect("fixture source-spec account");
    let verified_spec = verify_source_spec_account(
        PROGRAM_ID.to_bytes(),
        source_spec.address.to_bytes(),
        SourceSpecAccountViewV1::new(
            source_spec.address.to_bytes(),
            PROGRAM_ID.to_bytes(),
            false,
            &source_spec_bytes,
        ),
    )
    .expect("fixture source-spec authentication");
    let mut source_archive_bytes = vec![0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_archive::<FixtureDeployment>(
        &mut source_archive_bytes,
        verified_spec,
        window_domain,
        ArchivePredecessorV1::GENESIS,
        source_archive.bump,
    )
    .expect("fixture archive initialization");
    let provider = RuntimeAccountViewV1::new(
        SOURCE_PROGRAM,
        SOURCE_PROGRAM_OWNER,
        true,
        b"fixture-provider-v1",
    );
    let deployment = RuntimeAccountViewV1::new(
        SOURCE_DEPLOYMENT_ACCOUNT,
        SOURCE_DEPLOYMENT_OWNER,
        false,
        b"fixture-deployment-v1",
    );
    for (index, bucket) in (START_BUCKET..END_BUCKET).enumerate() {
        let record = fixture_source_record(bucket, index as u64 + 1, 1_000 + index as u64, 1, 0);
        append_authenticated::<FixturePriceParser, FixtureDeployment>(
            &mut source_archive_bytes,
            verified_spec,
            window_domain,
            TrustedClockV1 {
                slot: 1_005 + index as u64,
                unix_seconds: bucket * 60 + 1,
            },
            provider,
            deployment,
            SourceAccountView::new(SOURCE_DATA_ACCOUNT, SOURCE_PROGRAM, false, &record),
        )
        .expect("fixture authenticated source append");
    }
    seal_archive::<FixtureDeployment>(
        &mut source_archive_bytes,
        verified_spec,
        window_domain,
        FEED_CURSOR,
    )
    .expect("fixture source archive seal");

    let funded = !matches!(mode, Mode::Empty);
    let hoard_atoms = if funded { FUNDED_SETS } else { 0 };
    let lifecycle = u8::from(matches!(mode, Mode::Resolved(_)));
    let resolved_payout = match mode {
        Mode::Resolved(index) => index,
        _ => 0,
    };
    let supply_terms = if funded { internal } else { [0; MAX_OUTCOMES] };

    let mut accounts = vec![
        GenesisAccount {
            address: realm.address,
            owner: PROGRAM_ID,
            data: encode(account_len::REALM, |out| {
                RealmAccount {
                    realm: realm_id,
                    profile: profile_id,
                    max_outcomes: MAX_OUTCOMES as u8,
                    profile_version: 1,
                    stored_bump: realm.bump,
                    flags: 0,
                }
                .encode(out)
            }),
        },
        GenesisAccount {
            address: profile.address,
            owner: PROGRAM_ID,
            data: encode(account_len::PROFILE, |out| {
                ProfileAccount {
                    profile: profile_id,
                    realm: realm_id,
                    collateral_policy_digest: policy_digest,
                    version: 1,
                    flags: PROFILE_FLAG_POLICY_FROZEN,
                }
                .encode(out)
            }),
        },
        GenesisAccount {
            address: policy_artifact.address,
            owner: PROGRAM_ID,
            data: policy
                .canonical_bytes()
                .expect("the fixture policy must encode")
                .to_vec(),
        },
        GenesisAccount {
            address: terms.address,
            owner: PROGRAM_ID,
            data: encode(account_len::TERMS, |out| terms_account.encode(out)),
        },
        GenesisAccount {
            address: feed.address,
            owner: PROGRAM_ID,
            data: encode(account_len::FEED, |out| {
                FeedAccount {
                    feed: feed_id,
                    realm: realm_id,
                    cursor: FEED_CURSOR,
                    next_boundary: 0,
                    archive_pages: 0,
                    /* The codec refuses a zero summary: a feed head with no
                     * folded evidence behind it is not a cursor.  Nothing in
                     * the redemption path reads it. */
                    summary: Hash32::from_bytes([0x5a; 32]),
                    stored_bump: feed.bump,
                    flags: 0,
                }
                .encode(out)
            }),
        },
        GenesisAccount {
            address: source_spec.address,
            owner: PROGRAM_ID,
            data: source_spec_bytes,
        },
        GenesisAccount {
            address: source_archive.address,
            owner: PROGRAM_ID,
            data: source_archive_bytes,
        },
        GenesisAccount {
            address: BUFFER_ACCOUNT,
            owner: PROGRAM_ID,
            data: if matches!(mode, Mode::Resolved(_)) {
                evidence_buffer_bytes(window_id)
            } else {
                resolution_evidence_buffer_bytes(window_id, feed_id)
            },
        },
    ];

    if funded {
        accounts.extend([
            GenesisAccount {
                address: market.address,
                owner: PROGRAM_ID,
                data: encode(account_len::MARKET, |out| {
                    MarketAccount {
                        market: market_id,
                        realm: realm_id,
                        profile: profile_id,
                        terms: terms_id,
                        outcome_count: OUTCOME_COUNT,
                        lifecycle,
                        stored_bump: market.bump,
                        hoard_bump: hoard.bump,
                        outcomes,
                        feed: feed_id,
                        collateral_cap: COLLATERAL_CAP,
                        created_slot: 0,
                        reserved: Hash32::ZERO,
                    }
                    .encode(out)
                }),
            },
            GenesisAccount {
                address: hoard.address,
                owner: PROGRAM_ID,
                data: encode(account_len::HOARD, |out| {
                    HoardAccount {
                        market: market_id,
                        realm: realm_id,
                        authority: Hash32::from_bytes(hoard_authority.address.to_bytes()),
                        collateral_atoms: hoard_atoms,
                        stored_bump: hoard.bump,
                        flags: 0,
                    }
                    .encode(out)
                }),
            },
            GenesisAccount {
                address: position.address,
                owner: PROGRAM_ID,
                data: encode(account_len::POSITION, |out| {
                    PositionAccount {
                        market: market_id,
                        owner,
                        generation: GENERATION,
                        internal,
                        cash_atoms: CASH_ATOMS,
                        reserved_cash_atoms: RESERVED_CASH_ATOMS,
                        stored_bump: position.bump,
                        close_state: 0,
                    }
                    .encode(out)
                }),
            },
            GenesisAccount {
                address: kernel.address,
                owner: PROGRAM_ID,
                data: encode(KERNEL_ACCOUNT_LEN, |out| {
                    KernelAccount {
                        market: market_id,
                        phase: lifecycle,
                        basis_mode: BasisMode::FinitePreset,
                        resolved_payout,
                        payouts: payout_set(),
                        total_supply: internal,
                    }
                    .encode(out)
                }),
            },
            GenesisAccount {
                address: external.address,
                owner: PROGRAM_ID,
                data: encode(EXTERNAL_ACCOUNT_LEN, |out| {
                    ExternalAccount {
                        market: market_id,
                        owner,
                        position_generation: GENERATION,
                        balances: [0; MAX_OUTCOMES],
                        stored_bump: external.bump,
                        flags: 0,
                    }
                    .encode(out)
                }),
            },
            GenesisAccount {
                address: replay.address,
                owner: PROGRAM_ID,
                data: encode(REPLAY_ACCOUNT_LEN, |out| {
                    ReplayAccount {
                        market: market_id,
                        owner,
                        position_generation: GENERATION,
                        sequence: 0,
                        stored_bump: replay.bump,
                        flags: 0,
                    }
                    .encode(out)
                }),
            },
            GenesisAccount {
                address: supply.address,
                owner: PROGRAM_ID,
                data: encode(account_len::SUPPLY_LEDGER, |out| {
                    SupplyLedgerAccount {
                        market: market_id,
                        realm: realm_id,
                        generation: GENERATION,
                        outcome_count: OUTCOME_COUNT,
                        internal_supply: supply_terms,
                        external_supply: [0; MAX_OUTCOMES],
                        stored_bump: supply.bump,
                        flags: 0,
                    }
                    .encode(out)
                }),
            },
            GenesisAccount {
                address: resolution.address,
                owner: PROGRAM_ID,
                data: encode(account_len::RESOLUTION, |out| {
                    resolution_record(
                        market_id,
                        terms_id,
                        feed_id,
                        window_id,
                        &terms_account,
                        resolution.bump,
                        mode,
                    )
                    .encode(out)
                }),
            },
        ]);
    }

    Plane {
        realm_id,
        profile_id,
        market_id,
        actor,
        realm,
        profile,
        market,
        hoard,
        position,
        kernel,
        external,
        replay,
        supply,
        hoard_authority,
        hoard_token,
        outcome_mints,
        policy,
        policy_account: policy_artifact.address,
        collateral_mint,
        terms,
        terms_id,
        resolution,
        feed,
        feed_id,
        source_spec,
        source_archive,
        buffer: BUFFER_ACCOUNT,
        window_id,
        hoard_atoms,
        accounts,
    }
}

/// Replace a fixture plane's canonical archive with three authenticated records.
///
/// This is deliberately a **local SBF fixture** seam, not a production source
/// adapter. The rewritten account keeps the plane's exact SourceSpec, feed,
/// canonical window, PDA, bump, and sealed cursor. `confidence` must satisfy
/// the fixture SourceSpec's one-atom ceiling. Callers that also replace the
/// legacy Resolve evidence buffer must encode the exact resulting interval
/// `[price - confidence, price + confidence]` for all three buckets.
pub fn rewrite_plane_source_archive(plane: &mut Plane, price: u128, confidence: u128) {
    rewrite_plane_source_archive_span(plane, price, confidence, END_BUCKET - START_BUCKET);
}

/// Replace a fixture plane's canonical archive with an exact bounded span.
///
/// This focused profiling seam also updates the canonical window identity and
/// SourceArchive PDA. The caller must separately bind its Terms to the same
/// exclusive end before submitting a transaction. Existing three-record
/// scenarios use [`rewrite_plane_source_archive`] and remain byte-identical.
pub fn rewrite_plane_source_archive_span(
    plane: &mut Plane,
    price: u128,
    confidence: u128,
    span: u64,
) {
    assert!(
        confidence <= 1,
        "fixture confidence exceeds SourceSpec ceiling"
    );
    assert!(price >= confidence, "fixture interval underflows");
    assert!((1..=SOURCE_ARCHIVE_MAX_RECORDS_V1 as u64).contains(&span));
    let end_bucket = START_BUCKET
        .checked_add(span)
        .expect("fixture span end is representable");

    let spec_account = plane
        .accounts
        .iter()
        .find(|account| account.address == plane.source_spec.address)
        .expect("plane carries canonical SourceSpec");
    let verified_spec = verify_source_spec_account(
        PROGRAM_ID.to_bytes(),
        plane.source_spec.address.to_bytes(),
        SourceSpecAccountViewV1::new(
            plane.source_spec.address.to_bytes(),
            PROGRAM_ID.to_bytes(),
            false,
            &spec_account.data,
        ),
    )
    .expect("fixture SourceSpec remains authenticated");
    let feed_identity = FeedIdentity::new(SOURCE_ADAPTER_ID, plane.feed_id.bytes(), 7, 1)
        .expect("fixture source identity");
    let window_domain = WindowDomain::new(
        feed_identity,
        Grid::new(7, 1, 60).expect("fixture grid"),
        START_BUCKET,
        end_bucket,
        START_BUCKET + 4,
        0,
        CoveragePolicy::COMPLETE_REQUIRED,
    )
    .expect("fixture window domain");
    let window_id = canonical_window_id(window_domain);
    let source_archive = derive(&[
        seeds::SEED_SOURCE_ARCHIVE,
        &plane.feed_id.bytes(),
        &window_id.bytes(),
    ]);
    let old_archive_address = plane.source_archive.address;

    let mut archive = vec![0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_archive::<FixtureDeployment>(
        &mut archive,
        verified_spec,
        window_domain,
        ArchivePredecessorV1::GENESIS,
        source_archive.bump,
    )
    .expect("fixture archive initialization");
    let provider = RuntimeAccountViewV1::new(
        SOURCE_PROGRAM,
        SOURCE_PROGRAM_OWNER,
        true,
        b"fixture-provider-v1",
    );
    let deployment = RuntimeAccountViewV1::new(
        SOURCE_DEPLOYMENT_ACCOUNT,
        SOURCE_DEPLOYMENT_OWNER,
        false,
        b"fixture-deployment-v1",
    );
    for (index, bucket) in (START_BUCKET..end_bucket).enumerate() {
        let record = fixture_source_record(
            bucket,
            index as u64 + 1,
            1_000 + index as u64,
            price,
            confidence,
        );
        append_authenticated::<FixturePriceParser, FixtureDeployment>(
            &mut archive,
            verified_spec,
            window_domain,
            TrustedClockV1 {
                slot: 1_005 + index as u64,
                unix_seconds: bucket * 60 + 1,
            },
            provider,
            deployment,
            SourceAccountView::new(SOURCE_DATA_ACCOUNT, SOURCE_PROGRAM, false, &record),
        )
        .expect("fixture authenticated source append");
    }
    seal_archive::<FixtureDeployment>(&mut archive, verified_spec, window_domain, FEED_CURSOR)
        .expect("fixture source archive seal");
    let archive_account = plane
        .accounts
        .iter_mut()
        .find(|account| account.address == old_archive_address)
        .expect("plane carries canonical SourceArchive");
    archive_account.address = source_archive.address;
    archive_account.data = archive;
    plane.source_archive = source_archive;
    plane.window_id = window_id;
}

/// Encode one account into a freshly zeroed buffer of its exact length.
///
/// Generic over the error because the frozen layout codecs and the
/// reference-only codecs report through two different vocabularies; both are
/// `Debug`, and a fixture that does not encode is a bug in this harness rather
/// than a scenario.
fn encode<F, E>(len: usize, encoder: F) -> Vec<u8>
where
    F: FnOnce(&mut [u8]) -> Result<usize, E>,
    E: core::fmt::Debug,
{
    let mut out = vec![0_u8; len];
    let written = encoder(&mut out).expect("the fixture account must encode");
    assert_eq!(
        written, len,
        "encoder wrote a different length than the codec"
    );
    out
}

impl Plane {
    /// Canonical generation-zero Position and Replay addresses for any owner.
    pub fn owner_plane(&self, owner: Address) -> (Pda, Pda) {
        let market = self.market_id.bytes();
        let owner = owner.to_bytes();
        let generation = GENERATION.to_le_bytes();
        (
            derive(&[seeds::SEED_POSITION, &market, &owner]),
            derive(&[seeds::SEED_REPLAY, &market, &owner, &generation]),
        )
    }

    /// The seven program-owned targets `CreateMarket` must create from absent
    /// System-owned slots, in instruction order.
    pub fn market_state_addresses(&self) -> [Address; 7] {
        [
            self.market.address,
            self.hoard.address,
            self.position.address,
            self.kernel.address,
            self.replay.address,
            self.supply.address,
            self.resolution.address,
        ]
    }

    /// The nine seam state accounts, in the program's account-list order.
    pub fn seam_addresses(&self) -> [Address; 9] {
        [
            self.actor,
            self.realm.address,
            self.profile.address,
            self.market.address,
            self.hoard.address,
            self.position.address,
            self.kernel.address,
            self.replay.address,
            self.supply.address,
        ]
    }
}

/// Build the reference request envelope around one frozen layout intent.
///
/// The bytes are decoded back through the real
/// `clutch_solana_reference::Request::decode` before they are returned, so a
/// wrong envelope constant here is a panic in this harness rather than a
/// mysterious refusal from the program.
pub fn layout_request(sequence: u64, intent: Intent) -> Vec<u8> {
    let mut intent_bytes = [0_u8; MAX_INTENT_BYTES];
    let len = intent.encode(&mut intent_bytes).expect("intent encodes");
    let mut out = Vec::with_capacity(13 + len);
    out.push(REQUEST_TAG);
    out.push(REFERENCE_VERSION);
    out.extend_from_slice(&sequence.to_le_bytes());
    out.push(ACTION_LAYOUT);
    out.extend_from_slice(&(len as u16).to_le_bytes());
    out.extend_from_slice(&intent_bytes[..len]);
    let decoded = clutch_solana_reference::Request::decode(&out)
        .expect("this harness must emit an envelope the reference decodes");
    assert_eq!(decoded.sequence, sequence);
    out
}

/* ------------------------------------------------------------------------ */
/* Token-2022 account images                                                 */
/* ------------------------------------------------------------------------ */

/// Base (extension-free) Token-2022 mint length.
pub const BASE_MINT_LEN: usize = 82;
/// Base (extension-free) Token-2022 token-account length.
pub const BASE_TOKEN_ACCOUNT_LEN: usize = 165;

/// An outcome mint exactly as the plan proposes one: decimals `0`, `authority`
/// as mint authority, freeze authority `None`, no extensions.
///
/// These bytes are validated by the only authority that counts: the real
/// Token-2022 program executes `MintTo` and `Burn` against them.
pub fn outcome_mint_bytes(authority: Address, supply: u64) -> Vec<u8> {
    let mut data = vec![0_u8; BASE_MINT_LEN];
    data[0..4].copy_from_slice(&1_u32.to_le_bytes());
    data[4..36].copy_from_slice(&authority.to_bytes());
    data[36..44].copy_from_slice(&supply.to_le_bytes());
    data[44] = 0;
    data[45] = 1;
    data
}

/// The same mint carrying exactly one TLV extension entry, zero-valued.
///
/// Used to present the program with a mint that **decodes** and is refused:
/// the §3.4 check exists because a mint's *address* is not a description of
/// its behaviour, so the refusal has to come from reading the bytes.
///
/// `value_len` must be the extension's real value length, which the caller
/// computes from `ExtensionType::try_calculate_account_len` rather than from a
/// table — a short entry is a *malformed* mint, which the token program refuses
/// for the wrong reason and which would make this a test of the decoder rather
/// than of the policy.
pub fn mint_bytes_with_extension(
    authority: Address,
    supply: u64,
    discriminant: u16,
    value_len: usize,
) -> Vec<u8> {
    let base = outcome_mint_bytes(authority, supply);
    let mut data = vec![0_u8; BASE_TOKEN_ACCOUNT_LEN + 1 + 4 + value_len];
    data[..BASE_MINT_LEN].copy_from_slice(&base);
    data[BASE_TOKEN_ACCOUNT_LEN] = 1; // AccountType::Mint
    data[166..168].copy_from_slice(&discriminant.to_le_bytes());
    data[168..170].copy_from_slice(&(value_len as u16).to_le_bytes());
    data
}

/// TLV framing overhead of an extended mint: the base account length, the
/// account-type byte, and the four-byte TLV header.
pub const EXTENDED_MINT_OVERHEAD: usize = BASE_TOKEN_ACCOUNT_LEN + 1 + 4;

/* ------------------------------------------------------------------------ */
/* The evidence and terms plane                                              */
/* ------------------------------------------------------------------------ */

/// The account the caller-supplied evidence buffer is presented from.
///
/// Deliberately not derived: the buffer's *bytes* are the claim, not the
/// state, so binding its identity would suggest the bytes are trusted.
pub const BUFFER_ACCOUNT: Address = Address::new_from_array([0x8c; 32]);

/// Accepted feed cursor of the fixture feed head.
pub const FEED_CURSOR: u64 = 104;
/// First bucket of the fixture terms' exact expected range.
pub const START_BUCKET: u64 = 100;
/// Exclusive end bucket of that range.
pub const END_BUCKET: u64 = 103;

/// The immutable terms artifact both fixture markets bind.
///
/// One artifact, two markets: `require_terms_binds_market` compares the Realm,
/// Profile, feed and outcome count, and binds no market identity, so a terms
/// digest is a *shape* a market may be founded to rather than a per-market
/// record.
pub fn fixture_terms(realm: Hash32, profile: Hash32, feed: FeedId) -> TermsAccount {
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut left = [0; MAX_OUTCOMES];
    left[0] = 1;
    payouts[0] = PayoutVectorBytes {
        denominator: 1,
        weights: left,
    };
    let mut right = [0; MAX_OUTCOMES];
    right[1] = 1;
    payouts[1] = PayoutVectorBytes {
        denominator: 1,
        weights: right,
    };
    let mut knots = [0_u128; MAX_KNOTS];
    knots[0] = 1;
    let mut payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    payout_map[0] = 0;
    payout_map[1] = 1;
    let mut terms = TermsAccount {
        terms: Hash32::ZERO,
        realm,
        profile,
        feed,
        price_grid: Hash32::from_bytes([12; 32]),
        outcome_count: OUTCOME_COUNT,
        payout_count: 2,
        payouts,
        grid_family_id: 7,
        grid_version: 1,
        bucket_seconds: 60,
        expected_start_bucket: START_BUCKET,
        expected_end_bucket_exclusive: END_BUCKET,
        maturity_horizon_buckets: 4,
        coverage_policy_id: 1,
        repair_policy_id: 1,
        failure_policy_id: 1,
        statistic_id: 1,
        ambiguity_policy_id: 1,
        edge_policy_id: 1,
        basis_degree: 0,
        knot_count: 1,
        uniform_log2_spacing: UNIFORM_SPACING_NONE,
        failure_payout_index: 0,
        coverage_policy_parameter: 0,
        repair_generation: 0,
        source_version: 7,
        evaluator_version: 1,
        source_adapter_id: Hash32::from_bytes(SOURCE_ADAPTER_ID),
        payout_map,
        knots,
        collateral_cap: COLLATERAL_CAP,
        stored_bump: 255,
        flags: 0,
    };
    terms.terms = terms
        .recomputed_terms_digest()
        .expect("the fixture terms body encodes");
    terms
}

/// The resolution record a fixture plane installs.
fn resolution_record(
    market: Hash32,
    terms_id: Hash32,
    feed: FeedId,
    window: Hash32,
    terms: &TermsAccount,
    bump: u8,
    mode: Mode,
) -> ResolutionAccount {
    match mode {
        Mode::Resolved(payout_index) => ResolutionAccount {
            market,
            terms: terms_id,
            feed,
            window,
            feed_cursor: FEED_CURSOR,
            sealed_end_bucket_exclusive: terms.expected_end_bucket_exclusive,
            repair_generation: 0,
            resolved_slot: 0,
            payout_index,
            stored_bump: bump,
            flags: 0,
        },
        _ => ResolutionAccount {
            market,
            terms: terms_id,
            feed,
            window: Hash32::ZERO,
            feed_cursor: 0,
            sealed_end_bucket_exclusive: 0,
            repair_generation: 0,
            resolved_slot: 0,
            payout_index: PAYOUT_INDEX_UNRESOLVED,
            stored_bump: bump,
            flags: 0,
        },
    }
}

/// An evidence buffer carrying a window identity and **no** window bytes.
///
/// `RedeemInternal` refuses any window payload at all — its authority is the
/// recorded resolution, not a re-fold — so the empty payload is the only shape
/// a redemption may present.
fn evidence_buffer_bytes(window_id: Hash32) -> Vec<u8> {
    let mut data = vec![0_u8; EVIDENCE_BUFFER_HEADER_BYTES];
    data[0] = EVIDENCE_BUFFER_TAG;
    data[1] = BUFFER_VERSION;
    data[2..34].copy_from_slice(&window_id.bytes());
    data[34..36].copy_from_slice(&0_u16.to_le_bytes());
    data
}

/// An evidence buffer carrying the fixture's complete, mature sealed window.
///
/// The private `0x45` reference tag/version bytes are pinned here against the
/// production decoder: any drift makes the real SBF Resolve transaction fail.
fn resolution_evidence_buffer_bytes(window_id: Hash32, feed: FeedId) -> Vec<u8> {
    source_resolution_evidence_buffer(window_id, feed, 1, 1)
}

/// Encode the exact redundant Resolve projection for a local fixture archive.
///
/// The source-adapter identity is distinct from the content-addressed feed
/// identity.  Keeping this encoder next to [`rewrite_plane_source_archive`]
/// prevents tests from accidentally writing the feed digest into both fields,
/// a shape the canonical SourceSpec/window binding correctly refuses.
pub fn source_resolution_evidence_buffer(
    window_id: Hash32,
    feed: FeedId,
    low: u128,
    high: u128,
) -> Vec<u8> {
    let mut window = vec![0x45_u8, 1];
    window.extend_from_slice(&SOURCE_ADAPTER_ID); // source adapter
    window.extend_from_slice(&feed.bytes()); // feed spec
    window.extend_from_slice(&1_u32.to_le_bytes()); // source version
    window.extend_from_slice(&1_u32.to_le_bytes()); // evaluator version
    window.extend_from_slice(&7_u32.to_le_bytes()); // grid family
    window.extend_from_slice(&1_u16.to_le_bytes()); // grid version
    window.extend_from_slice(&60_u64.to_le_bytes()); // bucket seconds
    window.extend_from_slice(&START_BUCKET.to_le_bytes());
    window.extend_from_slice(&END_BUCKET.to_le_bytes());
    window.extend_from_slice(&(START_BUCKET + 4).to_le_bytes()); // maturity
    window.extend_from_slice(&0_u64.to_le_bytes()); // repair generation
    window.extend_from_slice(&1_u16.to_le_bytes()); // complete-required coverage
    window.extend_from_slice(&0_u64.to_le_bytes()); // coverage parameter
    window.extend_from_slice(&3_u16.to_le_bytes());
    for bucket in START_BUCKET..END_BUCKET {
        window.push(1); // accepted observation
        window.extend_from_slice(&bucket.to_le_bytes());
        window.extend_from_slice(&low.to_le_bytes());
        window.extend_from_slice(&high.to_le_bytes());
    }
    let mut data = vec![0_u8; EVIDENCE_BUFFER_HEADER_BYTES];
    data[0] = EVIDENCE_BUFFER_TAG;
    data[1] = BUFFER_VERSION;
    data[2..34].copy_from_slice(&window_id.bytes());
    data[34..36].copy_from_slice(&(window.len() as u16).to_le_bytes());
    data.extend_from_slice(&window);
    data
}

/* ------------------------------------------------------------------------ */
/* Account planes                                                            */
/* ------------------------------------------------------------------------ */

impl Plane {
    /// The fixed token-program and holder prefix of an outcome leg.
    pub fn outcome_leg(&self, holder: Address) -> [Address; 2] {
        [TOKEN_2022, holder]
    }

    /// The six accounts of the seam plane's collateral leg, in list order.
    pub fn collateral_leg(&self, actor_token: Address) -> [Address; 6] {
        [
            TOKEN_2022,
            self.policy_account,
            self.collateral_mint,
            actor_token,
            self.hoard_authority.address,
            self.hoard_token.address,
        ]
    }

    /// The eleven market-global prefix accounts of `Resolve`, in list order.
    ///
    /// Callers append every canonical outcome mint after this prefix.  Those
    /// mints are intentionally absent here because their count is a market
    /// fact rather than a fixture-wide constant.
    pub fn resolve_addresses(&self) -> [Address; 11] {
        [
            self.actor,
            self.market.address,
            self.hoard.address,
            self.kernel.address,
            self.supply.address,
            self.terms.address,
            self.resolution.address,
            self.feed.address,
            self.source_spec.address,
            self.source_archive.address,
            self.buffer,
        ]
    }

    /// The nine owner-scoped recorded-resolution accounts of `RedeemInternal`.
    ///
    /// Feed and caller evidence are intentionally absent after resolution.
    pub fn redeem_addresses(&self) -> [Address; 9] {
        [
            self.actor,
            self.market.address,
            self.hoard.address,
            self.position.address,
            self.kernel.address,
            self.replay.address,
            self.supply.address,
            self.terms.address,
            self.resolution.address,
        ]
    }

    /// The seven accounts `RedeemInternal` appends, in list order.
    pub fn redeem_leg(&self, actor_token: Address) -> [Address; 7] {
        [
            self.profile.address,
            TOKEN_2022,
            self.policy_account,
            self.collateral_mint,
            actor_token,
            self.hoard_authority.address,
            self.hoard_token.address,
        ]
    }

    /// Every account `CreateMarket` takes, in list order.
    pub fn create_market_addresses(&self, creator: Address) -> Vec<Address> {
        let mut out = vec![
            creator,
            self.realm.address,
            self.profile.address,
            self.terms.address,
            self.market.address,
            self.hoard.address,
            self.position.address,
            self.kernel.address,
            self.replay.address,
            self.supply.address,
            self.resolution.address,
            self.policy_account,
            TOKEN_2022,
            self.collateral_mint,
            SYSTEM_PROGRAM,
            RENT_SYSVAR,
            self.hoard_authority.address,
            self.hoard_token.address,
        ];
        out.extend(self.outcome_mints.iter().map(|mint| mint.address));
        out
    }
}

/// The System program's address: thirty-two zero bytes.
pub const SYSTEM_PROGRAM: Address = Address::new_from_array([0; 32]);
/// The Rent sysvar's address.
pub const RENT_SYSVAR: Address = Address::new_from_array([
    0x06, 0xa7, 0xd5, 0x17, 0x19, 0x2c, 0x5c, 0x51, 0x21, 0x8c, 0xc9, 0x4c, 0x3d, 0x4a, 0xf1, 0x7f,
    0x58, 0xda, 0xee, 0x08, 0x9b, 0xa1, 0xfd, 0x44, 0xe3, 0xdb, 0xd9, 0x8a, 0x00, 0x00, 0x00, 0x00,
]);

/// Exact byte length of a Token-2022 account carrying only `ImmutableOwner`.
pub const IMMUTABLE_OWNER_ACCOUNT_LEN: usize = BASE_TOKEN_ACCOUNT_LEN + 1 + 4;

/// A base Token-2022 account, exactly as the token program writes one.
pub fn token_account_bytes(mint: Address, owner: Address, amount: u64) -> Vec<u8> {
    let mut data = vec![0_u8; BASE_TOKEN_ACCOUNT_LEN];
    data[0..32].copy_from_slice(&mint.to_bytes());
    data[32..64].copy_from_slice(&owner.to_bytes());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1; // AccountState::Initialized
    data
}

/// The same, carrying the `ImmutableOwner` extension the Hoard is created with.
pub fn immutable_owner_account_bytes(mint: Address, owner: Address, amount: u64) -> Vec<u8> {
    let mut data = vec![0_u8; IMMUTABLE_OWNER_ACCOUNT_LEN];
    data[..BASE_TOKEN_ACCOUNT_LEN].copy_from_slice(&token_account_bytes(mint, owner, amount));
    data[BASE_TOKEN_ACCOUNT_LEN] = 2; // AccountType::Account
    data[166..168].copy_from_slice(&7_u16.to_le_bytes()); // ExtensionType::ImmutableOwner
    data[168..170].copy_from_slice(&0_u16.to_le_bytes());
    data
}

/// The `CreateMarket` intent the founding scenario submits.
pub fn create_market_request(plane: &Plane, nonce: u64) -> Vec<u8> {
    layout_request(
        0,
        Intent::CreateMarket {
            realm: plane.realm_id,
            profile: plane.profile_id,
            market_nonce: nonce,
            outcome_count: OUTCOME_COUNT,
            terms: plane.terms_id,
            feed: plane.feed_id,
        },
    )
}

/// The ComputeBudget program, used to raise the per-transaction unit ceiling.
///
/// `CreateMarket` founds a market *and* creates one Token-2022 mint per
/// outcome and the Hoard's token account, which is seven cross-program
/// invocations on top of a validated initialization write that already decoded
/// eight accounts twice. It does not fit the 200 000-unit default, and the
/// honest response to that is to say so and raise the ceiling in the
/// transaction rather than to stop measuring.
pub const COMPUTE_BUDGET: Address = Address::new_from_array([
    0x03, 0x06, 0x46, 0x6f, 0xe5, 0x21, 0x17, 0x32, 0xff, 0xec, 0xad, 0xba, 0x72, 0xc3, 0x9b, 0xe7,
    0xbc, 0x8c, 0xe5, 0xbb, 0xc5, 0xf7, 0x12, 0x6b, 0x2c, 0x43, 0x9b, 0x3a, 0x40, 0x00, 0x00, 0x00,
]);

/// `SetComputeUnitLimit(units)` instruction data.
pub fn compute_unit_limit_data(units: u32) -> Vec<u8> {
    let mut data = vec![0x02_u8];
    data.extend_from_slice(&units.to_le_bytes());
    data
}
