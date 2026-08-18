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
//! `ProgramTest` cannot create an account at a program-derived address from
//! outside: `system_instruction::create_account` needs the new account's
//! signature and only the owning program can sign for a PDA. So the program's
//! own state accounts *and* the outcome mints are placed at genesis with their
//! exact bytes, which is the same shape `SBF_BRINGUP.md`'s validator harness
//! already uses.
//!
//! For the outcome mints that is a real claim to defend: these are 82 bytes
//! this crate wrote, not bytes Token-2022 wrote. They are not taken on trust —
//! the real Token-2022 program is what executes `MintTo` and `Burn` against
//! them through the program's CPI, and it refuses anything it did not consider
//! a mint. A wrong byte here is a failing test, not a passing one.

#![deny(missing_docs)]

use clutch_kernel::{PayoutSet, PayoutVector};
use clutch_sbf::seeds;
use clutch_solana_layout::{
    account_len, canonical_market_id, canonical_outcome_id, canonical_profile_hash,
    canonical_realm_id, collateral, FeedId, Hash32, HoardAccount, Intent, MarketAccount,
    PositionAccount, ProfileAccount, RealmAccount, SupplyLedgerAccount, MAX_INTENT_BYTES,
    MAX_OUTCOMES, MAX_PAYOUTS, PROFILE_FLAG_POLICY_FROZEN, PROFILE_PARENT_BYTES,
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

/// Realm nonce, profile preimage fill, and the market nonce of the fixture.
const REALM_NONCE: u64 = 7;
const PROFILE_PREIMAGE_FILL: u8 = 0x2c;
const MARKET_NONCE: u64 = 9;
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
    /// The nine program-owned account images, in seam account-list order.
    pub accounts: Vec<GenesisAccount>,
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

/// The Realm's frozen collateral policy: a real, decodable 266-byte policy.
///
/// The collateral mint it names is a fixed fill rather than a live mint,
/// because no instruction in the seam plane reads it. The
/// `CreateMarket`-side admission that *would* read it is not wired — see the
/// status section of `docs/implementation/TOKEN2022_PLAN.md`.
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
pub fn build_plane(actor: Address) -> Plane {
    let profile_preimage = [PROFILE_PREIMAGE_FILL; PROFILE_PARENT_BYTES];
    let profile_id = canonical_profile_hash(&profile_preimage)
        .expect("the fixture profile preimage must be a canonical profile hash");
    let realm_id = canonical_realm_id(profile_id, REALM_NONCE);
    let market_id = canonical_market_id(realm_id, profile_id, MARKET_NONCE);
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

    let policy = fixture_policy([0x6d; 32]);
    let accounts = vec![
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
                    collateral_policy_digest: policy
                        .digest()
                        .expect("the fixture policy must digest"),
                    version: 1,
                    flags: PROFILE_FLAG_POLICY_FROZEN,
                }
                .encode(out)
            }),
        },
        GenesisAccount {
            address: market.address,
            owner: PROGRAM_ID,
            data: encode(account_len::MARKET, |out| {
                MarketAccount {
                    market: market_id,
                    realm: realm_id,
                    profile: profile_id,
                    terms: Hash32::from_bytes([0x7e; 32]),
                    outcome_count: OUTCOME_COUNT,
                    lifecycle: 0,
                    stored_bump: market.bump,
                    hoard_bump: hoard.bump,
                    outcomes,
                    feed: FeedId::from_bytes([9; 32]),
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
                    authority: Hash32::from_bytes(hoard.address.to_bytes()),
                    collateral_atoms: FUNDED_SETS,
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
                    phase: 0,
                    resolved_payout: 0,
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
                    internal_supply: internal,
                    external_supply: [0; MAX_OUTCOMES],
                    stored_bump: supply.bump,
                    flags: 0,
                }
                .encode(out)
            }),
        },
    ];

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
        accounts,
    }
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
    /// The ten seam accounts, in the program's account-list order.
    pub fn seam_addresses(&self) -> [Address; 10] {
        [
            self.actor,
            self.realm.address,
            self.profile.address,
            self.market.address,
            self.hoard.address,
            self.position.address,
            self.kernel.address,
            self.external.address,
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
