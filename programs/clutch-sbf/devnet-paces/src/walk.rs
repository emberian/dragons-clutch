//! The public-cluster walk plane: every address, body, and instruction of the
//! devnet paces campaign, derived against the *deployed* program id.
//!
//! This is the blank-bank public prefix of
//! `svm-tests/tests/joined_lifecycle.rs`, re-derived with the program id as a
//! parameter instead of the fixture constant.  Bodies (policy, grid, Terms,
//! `SourceSpec`) come from the same `clutch-solana-layout` / `clutch-sbf`
//! encoders the program decodes with; nothing here is a second description of
//! the protocol.

use clutch_sbf::instructions::{genesis, market_init, source_ingest};
use clutch_sbf::loader_state::UPGRADEABLE_LOADER_ID;
use clutch_sbf::seeds;
use clutch_sbf::source::{
    SourceSpecFieldsV1, SourceSpecV1, ORIENTATION_QUOTE_PER_BASE,
    SELECTION_FINALIZED_BUCKET_RECORD,
};
use clutch_collateral_adapter_v2::CollateralPolicyV2;
use clutch_solana_layout::{
    account_len,
    artifact::{ArtifactKind, ARTIFACT_CHUNK_BYTES},
    canonical_market_id, canonical_realm_id,
    canonical_profile_v2_id, Hash32, Intent, PriceGridAccount, TermsAccount, MAX_GRID_TICKS,
    MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS, PAYOUT_MAP_UNUSED, PayoutVectorBytes,
};
use clutch_svm_fixture::{
    compute_unit_limit_data, fixture_policy, fixture_terms, layout_request, COMPUTE_BUDGET,
    RENT_SYSVAR, SYSTEM_PROGRAM, TOKEN_2022,
};
use solana_address::Address;
use solana_instruction::{AccountMeta, Instruction};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// The Clock sysvar address.
pub const CLOCK_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);

/// The laboratory provider identities the compiled NON-PRODUCTION mock
/// registry names.  On a public cluster these accounts are unconstructible
/// (see `steps::devnet_impossible`); they appear in instructions only so the
/// deployed program's own authenticator can refuse them.
pub const MOCK_ADAPTER: [u8; 32] = [0xa1; 32];
pub const MOCK_PROGRAM: Address = Address::new_from_array([0xb2; 32]);
pub const MOCK_DEPLOYMENT: Address = Address::new_from_array([0xd4; 32]);
pub const MOCK_SOURCE: Address = Address::new_from_array([0xc3; 32]);
pub const DEPLOYMENT_GENERATION: u64 = 19;

/// Same walk constants as the joined lifecycle gate.
pub const REALM_NONCE: u64 = 7;
pub const MARKET_NONCE: u64 = 0x92;
pub const OUTCOMES: u8 = 4;
pub const SETS: u64 = 64;
pub const DENOMINATOR: u64 = 64;
/// The one smooth degree this campaign drives (the joined local gate covers
/// one, two, and three; the devnet paces walk needs a single market plane).
pub const DEGREE: u8 = 2;

fn token_2022_programdata() -> Address {
    Address::find_program_address(
        &[TOKEN_2022.as_ref()],
        &Address::new_from_array(UPGRADEABLE_LOADER_ID),
    )
    .0
}

/// The exact `SourceSpec` the compiled NON-PRODUCTION mock registry admits,
/// byte-for-byte the joined lifecycle's `walk_spec`.
pub fn walk_spec() -> Result<SourceSpecV1> {
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
    .map_err(|error| format!("walk source spec is not canonical: {error:?}").into())
}

/// Every derived address and encoded body of one campaign walk.
pub struct Walk {
    pub program_id: Address,
    pub payer: Address,
    pub actor: Address,
    pub bearer: Address,
    pub collateral_mint: Address,
    pub actor_token: Address,
    pub bearer_token: Address,

    pub policy: CollateralPolicyV2,
    pub policy_digest: Hash32,
    pub policy_body: Vec<u8>,
    pub profile_id: Hash32,
    pub realm_id: Hash32,
    pub grid_id: Hash32,
    pub grid_body: Vec<u8>,
    pub terms_value: TermsAccount,
    pub terms_id: Hash32,
    pub terms_body: Vec<u8>,
    pub spec: SourceSpecV1,
    pub feed_id: Hash32,
    pub market_id: Hash32,

    pub policy_account: Address,
    pub grid_account: Address,
    pub terms_account: Address,
    pub realm: Address,
    pub profile: Address,
    pub market: Address,
    pub hoard: Address,
    pub position: Address,
    pub kernel: Address,
    pub replay: Address,
    pub supply: Address,
    pub resolution: Address,
    pub hoard_authority: Address,
    pub hoard_token: Address,
    pub outcome_mints: [Address; 4],
    pub feed: Address,
    pub source_spec: Address,
    /// Canonical window identity of the walk's Terms; only the asserted
    /// archive refusal needs it (to target the canonical archive PDA).
    pub window_id: Hash32,
    pub source_archive: Address,
}

fn encode<F, E>(len: usize, encoder: F) -> Result<Vec<u8>>
where
    F: FnOnce(&mut [u8]) -> std::result::Result<usize, E>,
    E: core::fmt::Debug,
{
    let mut out = vec![0_u8; len];
    let written =
        encoder(&mut out).map_err(|error| format!("body does not encode: {error:?}"))?;
    if written != len {
        return Err(format!("encoder wrote {written} bytes into a {len}-byte body").into());
    }
    Ok(out)
}

impl Walk {
    /// Derive the walk against a deployed program id and fresh identities.
    ///
    /// `start_bucket`/`end_bucket` join the Terms to the mock-source window
    /// exactly as the local gate does; on a public cluster no append ever
    /// runs, so they only need to satisfy the frozen-terms window algebra.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        program_id: Address,
        payer: Address,
        actor: Address,
        bearer: Address,
        collateral_mint: Address,
        actor_token: Address,
        bearer_token: Address,
        start_bucket: u64,
        end_bucket: u64,
    ) -> Result<Self> {
        let derive = |seeds: &[&[u8]]| Address::find_program_address(seeds, &program_id);

        let policy = fixture_policy(collateral_mint.to_bytes());
        let policy_digest = Hash32::from_bytes(
            policy
                .id()
                .map_err(|error| format!("policy digest: {error:?}"))?
                .bytes(),
        );
        let release_id = Hash32::from_bytes(policy.adapter_release.bytes());
        let profile_id = canonical_profile_v2_id(policy_digest, release_id);
        let policy_body = policy
            .encode()
            .map_err(|error| format!("policy body: {error:?}"))?
            .to_vec();
        let realm_id = canonical_realm_id(profile_id, REALM_NONCE);

        let spec = walk_spec()?;
        let feed_id = spec.feed_id();

        let realm = derive(&[seeds::SEED_REALM, &realm_id.bytes()]).0;
        let profile =
            derive(&[seeds::SEED_PROFILE, &realm_id.bytes(), &profile_id.bytes()]).0;
        let policy_account = derive(&[
            seeds::SEED_POLICY,
            &profile_id.bytes(),
            &policy_digest.bytes(),
        ])
        .0;

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
        grid_value.grid = grid_value
            .recomputed_grid_id()
            .map_err(|error| format!("grid identity: {error:?}"))?;
        grid_value.stored_bump = derive(&[
            seeds::SEED_GRID,
            &realm_id.bytes(),
            &grid_value.grid.bytes(),
        ])
        .1;
        let grid_account = derive(&[
            seeds::SEED_GRID,
            &realm_id.bytes(),
            &grid_value.grid.bytes(),
        ])
        .0;
        let grid_body = encode(account_len::PRICE_GRID, |out| grid_value.encode(out))?;

        /* Degree-DEGREE smooth Terms, the exact shape of the joined local
         * walk: identity payouts over four outcomes, one-second buckets, the
         * window [start, end), and maturity exactly one bucket past the
         * exclusive end, as the source-ingest gate requires. */
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
        terms_value.basis_degree = DEGREE;
        terms_value.knot_count = OUTCOMES + 1 - DEGREE;
        terms_value.uniform_log2_spacing = 3;
        terms_value.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
        terms_value.knots = [0; MAX_KNOTS];
        for (index, knot) in terms_value
            .knots
            .iter_mut()
            .take(usize::from(terms_value.knot_count))
            .enumerate()
        {
            *knot = index as u128 * 8;
        }
        terms_value.grid_family_id = 5;
        terms_value.grid_version = 2;
        terms_value.bucket_seconds = 1;
        terms_value.expected_start_bucket = start_bucket;
        terms_value.expected_end_bucket_exclusive = end_bucket;
        terms_value.maturity_horizon_buckets = end_bucket - start_bucket + 1;
        terms_value.terms = Hash32::ZERO;
        terms_value.terms = terms_value
            .recomputed_terms_digest()
            .map_err(|error| format!("terms digest: {error:?}"))?;
        terms_value.stored_bump = derive(&[
            seeds::SEED_TERMS,
            &realm_id.bytes(),
            &terms_value.terms.bytes(),
        ])
        .1;
        let terms_id = terms_value.terms;
        let terms_account = derive(&[
            seeds::SEED_TERMS,
            &realm_id.bytes(),
            &terms_id.bytes(),
        ])
        .0;
        let terms_body = encode(account_len::TERMS, |out| terms_value.encode(out))?;

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
        let outcome_mints = [0_u8, 1, 2, 3].map(|outcome| {
            derive(&[seeds::SEED_OUTCOME_MINT, &market_id.bytes(), &[outcome]]).0
        });
        let feed = derive(&[seeds::SEED_FEED, &feed_id.bytes()]).0;
        let source_spec = derive(&[seeds::SEED_SOURCE_SPEC, &feed_id.bytes()]).0;

        let window_id = {
            use clutch_sbf::source_archive::{
                canonical_window_id, CoveragePolicy, FeedIdentity, Grid, WindowDomain,
            };
            let identity = FeedIdentity::new(MOCK_ADAPTER, feed_id.bytes(), 7, 1)
                .map_err(|error| format!("walk feed identity: {error:?}"))?;
            let window = WindowDomain::new(
                identity,
                Grid::new(5, 2, 1).map_err(|error| format!("walk grid: {error:?}"))?,
                start_bucket,
                end_bucket,
                end_bucket + 1,
                0,
                CoveragePolicy::COMPLETE_REQUIRED,
            )
            .map_err(|error| format!("walk window domain: {error:?}"))?;
            canonical_window_id(window)
        };
        let source_archive = derive(&[
            seeds::SEED_SOURCE_ARCHIVE,
            &feed_id.bytes(),
            &window_id.bytes(),
        ])
        .0;

        Ok(Self {
            program_id,
            payer,
            actor,
            bearer,
            collateral_mint,
            actor_token,
            bearer_token,
            policy,
            policy_digest,
            policy_body,
            profile_id,
            realm_id,
            grid_id: grid_value.grid,
            grid_body,
            terms_value,
            terms_id,
            terms_body,
            spec,
            feed_id,
            market_id,
            policy_account,
            grid_account,
            terms_account,
            realm,
            profile,
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
            feed,
            source_spec,
            window_id,
            source_archive,
        })
    }

    pub fn budget() -> Instruction {
        Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(1_400_000), vec![])
    }

    fn stage_address(&self, kind: ArtifactKind, context: Hash32, digest: Hash32) -> Address {
        Address::find_program_address(
            &[
                seeds::SEED_ARTIFACT_STAGE,
                self.payer.as_ref(),
                &[kind.byte()],
                &context.bytes(),
                &digest.bytes(),
            ],
            &self.program_id,
        )
        .0
    }

    /// The (kind, context, digest, final address, stage address, body) of one
    /// artifact upload, keyed by a stable route name.
    pub fn artifact_route(
        &self,
        route: ArtifactRoute,
    ) -> (ArtifactKind, Hash32, Hash32, Address, Address, &[u8]) {
        let (kind, context, digest, final_account, body): (_, _, _, _, &[u8]) = match route {
            ArtifactRoute::Policy => (
                ArtifactKind::CollateralPolicy,
                self.profile_id,
                self.policy_digest,
                self.policy_account,
                &self.policy_body,
            ),
            ArtifactRoute::Grid => (
                ArtifactKind::PriceGrid,
                self.realm_id,
                self.grid_id,
                self.grid_account,
                &self.grid_body,
            ),
            ArtifactRoute::Terms => (
                ArtifactKind::Terms,
                self.realm_id,
                self.terms_id,
                self.terms_account,
                &self.terms_body,
            ),
        };
        let stage = self.stage_address(kind, context, digest);
        (kind, context, digest, final_account, stage, body)
    }

    pub fn artifact_begin(&self, route: ArtifactRoute, expires_slot: u64) -> Instruction {
        let (kind, context, digest, _, stage, body) = self.artifact_route(route);
        debug_assert_eq!(kind.exact_len(), body.len());
        Instruction::new_with_bytes(
            self.program_id,
            &layout_request(
                0,
                Intent::BeginArtifact {
                    kind,
                    context,
                    digest,
                    exact_len: u16::try_from(kind.exact_len()).expect("artifact length fits"),
                    expires_slot,
                },
            ),
            vec![
                AccountMeta::new(self.payer, true),
                AccountMeta::new(stage, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR, false),
            ],
        )
    }

    pub fn artifact_write(&self, route: ArtifactRoute, cursor: usize) -> Instruction {
        let (kind, context, digest, _, stage, body) = self.artifact_route(route);
        let chunk_len = ARTIFACT_CHUNK_BYTES.min(body.len() - cursor);
        let mut chunk = [0_u8; ARTIFACT_CHUNK_BYTES];
        chunk[..chunk_len].copy_from_slice(&body[cursor..cursor + chunk_len]);
        Instruction::new_with_bytes(
            self.program_id,
            &layout_request(
                0,
                Intent::WriteArtifact {
                    kind,
                    context,
                    digest,
                    cursor: u16::try_from(cursor).expect("artifact cursor fits"),
                    chunk_len: u16::try_from(chunk_len).expect("artifact chunk fits"),
                    chunk,
                },
            ),
            vec![
                AccountMeta::new_readonly(self.payer, true),
                AccountMeta::new(stage, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR, false),
            ],
        )
    }

    pub fn artifact_seal(&self, route: ArtifactRoute) -> Instruction {
        let (kind, context, digest, final_account, stage, _) = self.artifact_route(route);
        Instruction::new_with_bytes(
            self.program_id,
            &layout_request(
                0,
                Intent::SealArtifact {
                    kind,
                    context,
                    digest,
                    exact_len: u16::try_from(kind.exact_len()).expect("artifact length fits"),
                },
            ),
            vec![
                AccountMeta::new(self.payer, true),
                AccountMeta::new(stage, false),
                AccountMeta::new(final_account, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(CLOCK_SYSVAR, false),
            ],
        )
    }

    pub fn init_realm(&self) -> Instruction {
        Instruction::new_with_bytes(
            self.program_id,
            &layout_request(
                0,
                Intent::InitRealm {
                    profile: self.profile_id,
                    realm_nonce: REALM_NONCE,
                    max_outcomes: u8::try_from(MAX_OUTCOMES).expect("outcome bound fits"),
                    profile_version: 2,
                },
            ),
            vec![
                AccountMeta::new(self.payer, true),
                AccountMeta::new(self.realm, false),
                AccountMeta::new_readonly(self.policy_account, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
            ],
        )
    }

    pub fn init_profile(&self) -> Instruction {
        Instruction::new_with_bytes(
            self.program_id,
            &layout_request(
                0,
                Intent::InitProfileV2 {
                    realm: self.realm_id,
                    collateral_policy_id: self.policy_digest,
                    adapter_release_id: Hash32::from_bytes(self.policy.adapter_release.bytes()),
                    profile_version: 2,
                },
            ),
            vec![
                AccountMeta::new(self.payer, true),
                AccountMeta::new(self.profile, false),
                AccountMeta::new_readonly(self.realm, false),
                AccountMeta::new_readonly(self.policy_account, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
                AccountMeta::new_readonly(RENT_SYSVAR, false),
                AccountMeta::new_readonly(TOKEN_2022, false),
                AccountMeta::new_readonly(token_2022_programdata(), false),
            ],
        )
    }

    pub fn create_market(&self) -> Instruction {
        let mut metas = vec![
            AccountMeta::new(self.actor, true),
            AccountMeta::new_readonly(self.realm, false),
            AccountMeta::new_readonly(self.profile, false),
            AccountMeta::new_readonly(self.terms_account, false),
            AccountMeta::new(self.market, false),
            AccountMeta::new(self.hoard, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.kernel, false),
            AccountMeta::new(self.replay, false),
            AccountMeta::new(self.supply, false),
            AccountMeta::new(self.resolution, false),
            AccountMeta::new_readonly(self.policy_account, false),
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
            self.program_id,
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

    pub fn init_source_spec(&self) -> Instruction {
        let metas = vec![
            AccountMeta::new(self.payer, true),
            AccountMeta::new(self.source_spec, false),
            AccountMeta::new(self.feed, false),
            AccountMeta::new_readonly(self.terms_account, false),
            AccountMeta::new_readonly(MOCK_PROGRAM, false),
            AccountMeta::new_readonly(MOCK_DEPLOYMENT, false),
            AccountMeta::new_readonly(MOCK_SOURCE, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(metas.len(), source_ingest::INIT_SOURCE_SPEC_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            self.program_id,
            &layout_request(
                0,
                Intent::InitSourceSpec {
                    terms: self.terms_id,
                    spec_body: self.spec.encode_canonical(),
                },
            ),
            metas,
        )
    }

    pub fn init_source_archive(&self) -> Instruction {
        let metas = vec![
            AccountMeta::new(self.payer, true),
            AccountMeta::new_readonly(self.source_spec, false),
            AccountMeta::new_readonly(self.feed, false),
            AccountMeta::new_readonly(self.terms_account, false),
            AccountMeta::new(self.source_archive, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ];
        assert_eq!(
            metas.len(),
            source_ingest::INIT_SOURCE_ARCHIVE_ACCOUNT_COUNT
        );
        Instruction::new_with_bytes(
            self.program_id,
            &layout_request(0, Intent::InitSourceArchive { terms: self.terms_id }),
            metas,
        )
    }

    pub fn endow(&self, sequence: u64, amount: u64) -> Instruction {
        let metas = vec![
            AccountMeta::new(self.actor, true),
            AccountMeta::new_readonly(self.market, false),
            AccountMeta::new_readonly(self.hoard, false),
            AccountMeta::new(self.position, false),
            AccountMeta::new(self.replay, false),
            AccountMeta::new_readonly(self.profile, false),
            AccountMeta::new_readonly(self.policy_account, false),
            AccountMeta::new_readonly(TOKEN_2022, false),
            AccountMeta::new_readonly(self.collateral_mint, false),
            AccountMeta::new(self.actor_token, false),
            AccountMeta::new(self.hoard_token, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(self.terms_account, false),
            AccountMeta::new_readonly(self.source_spec, false),
        ];
        assert_eq!(metas.len(), genesis::ENDOW_ACCOUNT_COUNT);
        Instruction::new_with_bytes(
            self.program_id,
            &layout_request(
                sequence,
                Intent::Endow {
                    market: self.market_id,
                    owner: Hash32::from_bytes(self.actor.to_bytes()),
                    amount,
                },
            ),
            metas,
        )
    }
}

/// The three artifact uploads of the public prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactRoute {
    Policy,
    Grid,
    Terms,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_walk() -> Walk {
        Walk::build(
            Address::new_from_array([0x11; 32]),
            Address::new_from_array([0x21; 32]),
            Address::new_from_array([0x22; 32]),
            Address::new_from_array([0x23; 32]),
            Address::new_from_array([0x24; 32]),
            Address::new_from_array([0x25; 32]),
            Address::new_from_array([0x26; 32]),
            1_000,
            1_002,
        )
        .expect("walk builds")
    }

    #[test]
    fn the_walk_spec_is_the_registered_mock_release_shape() {
        let spec = walk_spec().expect("walk spec is canonical");
        assert_eq!(spec.source_adapter_id().bytes(), MOCK_ADAPTER);
        assert_eq!(spec.source_adapter_version(), 7);
        assert_eq!(spec.parser_id(), 11);
        assert_eq!(spec.parser_version(), 3);
        assert_eq!(spec.source_program(), MOCK_PROGRAM.to_bytes());
        assert_eq!(spec.source_account(), MOCK_SOURCE.to_bytes());
        assert_eq!(spec.deployment_generation(), DEPLOYMENT_GENERATION);
    }

    #[test]
    fn terms_bind_the_walk_feed_and_recompute_their_own_digest() {
        let walk = test_walk();
        assert_eq!(walk.terms_value.feed, walk.feed_id);
        assert_eq!(walk.terms_value.basis_degree, DEGREE);
        assert_eq!(
            walk.terms_value.recomputed_terms_digest().expect("digest"),
            walk.terms_id
        );
        assert_eq!(
            walk.terms_value.maturity_horizon_buckets,
            walk.terms_value.expected_end_bucket_exclusive
                - walk.terms_value.expected_start_bucket
                + 1
        );
    }

    #[test]
    fn derived_addresses_depend_on_the_program_id() {
        let one = test_walk();
        let two = Walk::build(
            Address::new_from_array([0x12; 32]),
            one.payer,
            one.actor,
            one.bearer,
            one.collateral_mint,
            one.actor_token,
            one.bearer_token,
            1_000,
            1_002,
        )
        .expect("walk builds");
        assert_ne!(one.market, two.market);
        assert_ne!(one.source_spec, two.source_spec);
        assert_eq!(one.market_id, two.market_id);
    }

    #[test]
    fn artifact_bodies_have_their_kinds_exact_lengths() {
        let walk = test_walk();
        for route in [ArtifactRoute::Policy, ArtifactRoute::Grid, ArtifactRoute::Terms] {
            let (kind, _, _, _, _, body) = walk.artifact_route(route);
            assert_eq!(kind.exact_len(), body.len(), "{route:?}");
        }
    }

    #[test]
    fn instruction_account_counts_match_the_program_constants() {
        let walk = test_walk();
        assert_eq!(
            walk.create_market().accounts.len(),
            market_init::account_count(OUTCOMES)
        );
        assert_eq!(
            walk.init_source_spec().accounts.len(),
            source_ingest::INIT_SOURCE_SPEC_ACCOUNT_COUNT
        );
        assert_eq!(
            walk.init_source_archive().accounts.len(),
            source_ingest::INIT_SOURCE_ARCHIVE_ACCOUNT_COUNT
        );
        assert_eq!(
            walk.endow(0, SETS).accounts.len(),
            genesis::ENDOW_ACCOUNT_COUNT
        );
    }
}
