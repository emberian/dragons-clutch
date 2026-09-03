//! Width-parameterized Product Runtime V3/LBV2 fixture for Fractional.
//!
//! Shared verbatim with the ProgramTest campaign at
//! `programs/dclutch-claims-sbf/program-test/fractional-atomic/src/`. The two
//! crates are separate workspaces on different Solana lineages, so the geometry
//! is duplicated rather than linked; the exterior asserts the fixture it stages
//! is byte-identical to the one the ProgramTest campaign proved, which is what
//! makes that duplication safe to have.
//!
//! The shared campaign fixture in `dclutch-claims-affine-batch-program-test` is
//! fixed at 258 outcomes, which is permanently out of Fractional range: a
//! Fractional capability names one shard Mint per representation coordinate and
//! [`FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2`] bounds that at 256, because the
//! action selector the Market dispatches on is a `U8` at a fixed request offset.
//! That bound is deliberate, so this module re-derives the same canonical
//! geometry at a caller-selected width instead of widening the terms or
//! disturbing the shared 258-outcome fixture every other Claims campaign pins.
//!
//! Everything here is the same canonical encoding the shared fixture emits --
//! the Registry-owned `ProductBasisV3` record, the Product Runtime V2 graph, the
//! Core state, and the LBV2 aggregate and Positions -- only with the outcome
//! count lifted into a parameter.

use dclutch_claims_svm::protocol_position_v2::ProtocolPositionSeedsV2;
use dclutch_market_core_codec::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, ProductGraphBumpsV1,
    Readiness, STATE_BYTES, StateBumpsV1,
};

use dclutch_product_payoff_v2_codec::{
    price_gate_v1::verify_price_gate_v1,
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product_runtime_v2::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_representation_composition_v3_kernel::{
    COMPOSITION_EXPOSURE_SCHEMA_ID_V3, CompositionExposureInputV3, CompositionExposureRowInputV3,
    CompositionExposureTermV3, composition_exposure_bytes_v3,
    encode_composition_exposure_v3_atomic,
};
use solana_program::{
    hash::{hash, hashv},
    pubkey::Pubkey,
};

/// The exact representation width a Fractional capability may name.
///
/// One shard Mint per representation coordinate, indexed by the `U8` action
/// selector the Market dispatches on, bounds the space at 256.
pub const FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2: usize = 256;

const CLAIMS_MARKET_MAGIC_V2: [u8; 8] = *b"DCLLBM02";
const CLAIMS_POSITION_MAGIC_V2: [u8; 8] = *b"DCLLBP02";
const CLAIMS_ABI_VERSION_V2: u16 = 2;
const CLAIMS_MARKET_HEADER_BYTES_V2: usize = 256;
const CLAIMS_POSITION_HEADER_BYTES_V2: usize = 128;
// Taken from the crate that owns the domain rather than restated. The bytes
// are one address's identity and a second declaration of them is a second
// author: `dclutch:lbv2:market` stood under two names -- this one and the
// owner's -- until the seam register's DOMAIN_BYTES_COLLIDE said so.
use dclutch_claims_svm::liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2;
/// Product coordinate-domain identity this fixture compiles against.
pub const COORDINATE_DOMAIN_ID: [u8; 32] = [0x43; 32];
/// Product result-unit identity this fixture compiles against.
pub const RESULT_UNIT_ID: [u8; 32] = [0x44; 32];
/// Product evaluator release this fixture compiles against.
pub const EVALUATOR_RELEASE_ID: [u8; 32] = [0x48; 32];
/// Payout scale the categorical basis is compiled with.
///
/// Read by the terminal derivation in the ProgramTest campaign; the exterior
/// carries it so the two geometries stay one definition.
#[allow(dead_code)]
pub const PAYOUT_SCALE: u64 = 1;
const PROVISIONAL_RESULT_DOMAIN_ID: [u8; 32] = [0x49; 32];

/// Exact degree-2/3 curve inputs accepted by the narrow fixture compiler.
///
/// The compiler verifies `price_gate_certificate` against a canonical probe
/// basis before its digest becomes part of the semantic and finalized basis.
/// Callers therefore cannot stage a live curve by merely asserting a digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NarrowSplineBasisInputV3<'a> {
    /// Cox-de Boor degree; the live profile admits only two or three.
    pub degree: u8,
    /// Whether repeated interior knots are part of this Product's profile.
    pub interior_multiplicity: bool,
    /// Exact payout partition scale.
    pub payout_scale: u64,
    /// Shared positive denominator for every knot numerator.
    pub knot_denominator: u64,
    /// Canonical nondecreasing knot numerators.
    pub knots: &'a [i128],
    /// Exact failure-region payout partition.
    pub failure_payouts: &'a [u64],
    /// Canonical `DCLTPGT1` certificate bytes.
    pub price_gate_certificate: &'a [u8],
}

/// Liability-basis family selected at the compiler-shaped entrance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NarrowBasisInputV3<'a> {
    /// Exact categorical `Q = 1` basis.
    Categorical,
    /// Live degree-2/3 spline basis with an admitted price gate.
    SplineDegree2To3(NarrowSplineBasisInputV3<'a>),
}

/// Narrow-fixture compilation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NarrowFixtureError {
    /// A nonzero identity refused construction.
    Identity,
    /// Runtime Product compilation or exact width refused.
    Product,
    /// Canonical ProductBasisV3 encoding refused.
    Basis,
    /// Core or Claims fixed-layout encoding refused.
    State,
    /// Canonical composition-exposure encoding refused.
    Exposure,
    /// The requested outcome width is not usable by a Fractional capability.
    Width,
}

/// Result alias for narrow-fixture compilation.
pub type Result<T> = core::result::Result<T, NarrowFixtureError>;

/// Exact finalized-record body and derived account keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NarrowRecordV2 {
    /// Program that owns the finalized raw record.
    pub owner: Pubkey,
    /// Exact schema identity used in raw/staging PDA derivation.
    pub schema: [u8; 32],
    /// SHA-256 of the complete raw body.
    pub digest: [u8; 32],
    /// Canonical raw-record PDA.
    pub raw: Pubkey,
    /// Canonical vacant staging PDA.
    pub staging: Pubkey,
    /// Bumps the two derivations above found, kept rather than discarded.
    ///
    /// A founded Market records the Product graph's four record pairs in its
    /// `StateBumpsV1`, and its readers reproduce each address from the recorded
    /// bump instead of searching. A fixture that leaves the tail unrecorded is
    /// not staging a neutral default -- it is staging a market no founding
    /// produces, and every compute measurement taken on it measures the search.
    pub raw_bump: u8,
    /// See [`NarrowRecordV2::raw_bump`].
    pub staging_bump: u8,
    /// Complete raw body.
    pub bytes: Vec<u8>,
}

/// One canonical LBV2 Position.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NarrowPositionV2 {
    /// Sole semantic Position owner.
    pub owner: Pubkey,
    /// Canonical Claims Position PDA.
    pub account: Pubkey,
    /// Complete runtime-width Position bytes.
    pub bytes: Vec<u8>,
}

/// A resolved Market: the Core state carries a terminal receipt and a winner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NarrowTerminalInputV2 {
    /// Winning representation coordinate.
    pub winner: u32,
    /// Core terminal receipt identity.
    pub receipt: [u8; 32],
}

/// Immutable inputs selecting one narrow fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NarrowFixtureInputV2 {
    /// Exact runtime outcome width.
    pub outcome_count: usize,
    /// Registry program owning Product Runtime V2 records.
    pub registry_program: Pubkey,
    /// Core program owning the Market state.
    pub core_program: Pubkey,
    /// Claims program owning the aggregate and Positions.
    pub claims_program: Pubkey,
    /// Current execution release set.
    pub release_set: [u8; 32],
    /// Immutable Realm identity.
    pub realm_id: [u8; 32],
    /// Immutable Custody replay namespace.
    pub custody_context: [u8; 32],
    /// Market generation.
    pub generation: u64,
    /// Actor Position owner, funded at the selected coordinate.
    pub actor_owner: Pubkey,
    /// Reserve Position owner, initially empty.
    pub reserve_owner: Pubkey,
    /// Representation coordinate the campaign funds.
    pub funded_coordinate: usize,
    /// Native Claims units the actor holds at that coordinate.
    pub funded_balance: u64,
    /// Replay revision both Positions open at.
    ///
    /// Claims compares a Position's revision for exact equality against the
    /// packet's expectation, so a campaign that needs Positions with history --
    /// anything already traded against -- cannot use a fixture pinned at zero.
    /// Suggested by the Dealer lane, which hit exactly that.
    pub position_revision: u64,
    /// Native Claims already locked in the reserve Position at that coordinate.
    ///
    /// A terminal campaign opens after a wrap has happened, so the reserve is
    /// not empty; this is how that already-wrapped state is expressed.
    pub reserve_balance: u64,
    /// When set, the Market is resolved rather than open.
    pub terminal: Option<NarrowTerminalInputV2>,
    /// Permanent Core rent beneficiary.
    ///
    /// Custody refuses an aliased account frame, so this must not be the payer
    /// that funds the replay cursor.
    pub rent_beneficiary: Pubkey,
    /// Stable source composition-graph identity, shared with the terms record.
    pub graph_id: [u8; 32],
    /// Caller-asserted exposure bundle identity, shared with the terms record.
    pub exposure_id: [u8; 32],
}

/// Complete canonical narrow fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NarrowFixtureV2 {
    /// Exact runtime outcome width.
    pub outcome_count: u32,
    /// Stable semantic Product identity.
    pub product_id: [u8; 32],
    /// Semantic categorical-Q=1 liability basis identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact payout scale persisted by the finalized ProductBasisV3.
    pub payout_scale: u64,
    /// Canonical Product graph-root record.
    pub product: NarrowRecordV2,
    /// Canonical Product-selected result-domain record.
    pub result_domain: NarrowRecordV2,
    /// Canonical Product-selected portfolio record.
    pub portfolio: NarrowRecordV2,
    /// Canonical Product-linked categorical LBV2 record.
    pub linked_basis: NarrowRecordV2,
    /// Canonical open Core Market PDA.
    pub core_market: Pubkey,
    /// Complete Core state bytes.
    pub core_state: Vec<u8>,
    /// Canonical LBV2 aggregate PDA.
    pub claims_market: Pubkey,
    /// Complete aggregate bytes.
    pub claims_market_bytes: Vec<u8>,
    /// Canonical composition-exposure record: the identity Claims translation
    /// from Product result coordinates onto Claims representation roots.
    pub exposure: NarrowRecordV2,
    /// Caller-asserted exposure bundle identity.
    pub exposure_id: [u8; 32],
    /// Actor Position, funded at the selected coordinate.
    pub actor_position: NarrowPositionV2,
    /// Reserve Position, initially empty.
    pub reserve_position: NarrowPositionV2,
}

impl NarrowFixtureV2 {
    /// The two Positions in the order Claims recomputes them: sorted by owner.
    #[must_use]
    pub fn ordered_positions(&self) -> [&NarrowPositionV2; 2] {
        if self.actor_position.owner.to_bytes() < self.reserve_position.owner.to_bytes() {
            [&self.actor_position, &self.reserve_position]
        } else {
            [&self.reserve_position, &self.actor_position]
        }
    }
}

/// Compile one canonical Product Runtime V2/LBV2 fixture at a chosen width.
pub fn compile_narrow_fixture_v2(input: NarrowFixtureInputV2) -> Result<NarrowFixtureV2> {
    compile_narrow_fixture_v3(input, NarrowBasisInputV3::Categorical)
}

/// Compile one canonical Product Runtime V2/LBV2 fixture with an explicitly
/// selected ProductBasisV3 family.
pub fn compile_narrow_fixture_v3(
    input: NarrowFixtureInputV2,
    basis_input: NarrowBasisInputV3<'_>,
) -> Result<NarrowFixtureV2> {
    // Two outcomes are the closed tails either side of the cut vector, so a
    // usable Product needs at least one interior cut.
    if input.outcome_count < 3
        || input.outcome_count > FRACTIONAL_MAX_REPRESENTATION_WIDTH_V2
        || input.funded_coordinate >= input.outcome_count
        || input.release_set == [0; 32]
        || input.realm_id == [0; 32]
        || input.custody_context == [0; 32]
        || input.actor_owner == input.reserve_owner
    {
        return Err(NarrowFixtureError::Width);
    }
    let product_id = content([0x41; 32])?;
    let outcome_width =
        u32::try_from(input.outcome_count).map_err(|_| NarrowFixtureError::Width)?;
    let basis_width = u32::try_from(input.outcome_count).map_err(|_| NarrowFixtureError::Basis)?;
    let (kind, payout_scale, knot_denominator, knots, failure_payouts, gate_bytes) =
        match basis_input {
            NarrowBasisInputV3::Categorical => (
                BasisKindV3::CategoricalQ1,
                PAYOUT_SCALE,
                1,
                &[][..],
                &[][..],
                &[][..],
            ),
            NarrowBasisInputV3::SplineDegree2To3(spline) => (
                BasisKindV3::SplineDegree2To3 {
                    degree: spline.degree,
                    interior_multiplicity: spline.interior_multiplicity,
                },
                spline.payout_scale,
                spline.knot_denominator,
                spline.knots,
                spline.failure_payouts,
                spline.price_gate_certificate,
            ),
        };
    let basis_bytes = basis_record_bytes_v3(kind, input.outcome_count, knots.len(), 0)
        .map_err(|_| NarrowFixtureError::Basis)?;
    let price_gate_certificate_digest = match basis_input {
        NarrowBasisInputV3::Categorical => [0_u8; 32],
        NarrowBasisInputV3::SplineDegree2To3(spline) => {
            let mut probe = vec![0_u8; basis_bytes];
            compile_basis_v3(
                BasisInputV3 {
                    kind,
                    product_id: product_id.to_bytes(),
                    result_domain_id: PROVISIONAL_RESULT_DOMAIN_ID,
                    coordinate_domain_id: COORDINATE_DOMAIN_ID,
                    result_unit_id: RESULT_UNIT_ID,
                    evaluator_release_id: EVALUATOR_RELEASE_ID,
                    basis_width,
                    payout_scale,
                    knot_denominator,
                    knots,
                    terms: &[],
                    failure_payouts,
                    // The certificate does not depend on its own digest. A
                    // nonzero probe unlocks decoding; the verified hash below
                    // is what the staged record actually persists.
                    price_gate_certificate_digest: [1_u8; 32],
                },
                &mut probe,
            )
            .map_err(|_| NarrowFixtureError::Basis)?;
            let basis = ProductBasisV3::decode(&probe).map_err(|_| NarrowFixtureError::Basis)?;
            verify_price_gate_v1(
                &basis,
                knot_denominator,
                payout_scale,
                spline.degree,
                basis_width,
                gate_bytes,
            )
            .map_err(|_| NarrowFixtureError::Basis)?;
            hash(gate_bytes).to_bytes()
        }
    };
    let provisional_basis_input = BasisInputV3 {
        kind,
        product_id: product_id.to_bytes(),
        result_domain_id: PROVISIONAL_RESULT_DOMAIN_ID,
        coordinate_domain_id: COORDINATE_DOMAIN_ID,
        result_unit_id: RESULT_UNIT_ID,
        evaluator_release_id: EVALUATOR_RELEASE_ID,
        basis_width,
        payout_scale,
        knot_denominator,
        knots,
        terms: &[],
        failure_payouts,
        price_gate_certificate_digest,
    };
    let mut provisional_basis = vec![0_u8; basis_bytes];
    compile_basis_v3(provisional_basis_input, &mut provisional_basis)
        .map_err(|_| NarrowFixtureError::Basis)?;
    let semantic_basis_id = semantic_basis_id_v3(&provisional_basis)?;

    let cut_count = input
        .outcome_count
        .checked_sub(2)
        .ok_or(NarrowFixtureError::Width)?;
    let cuts: Vec<i128> = (0..cut_count)
        .map(|value| i128::try_from(value).unwrap_or_default())
        .collect();
    let coefficients = vec![1_u64; input.outcome_count];
    let (product_bytes, domain_bytes, portfolio_bytes) = compile_product(
        input.registry_program,
        product_id,
        semantic_basis_id,
        &cuts,
        &coefficients,
    )?;
    let product = finalized(
        input.registry_program,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        product_bytes,
    );
    let result_domain = finalized(
        input.registry_program,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        domain_bytes,
    );
    let portfolio = finalized(
        input.registry_program,
        PORTFOLIO_SCHEMA_ID_V2,
        portfolio_bytes,
    );
    let linked_basis_bytes = {
        let mut output = vec![0_u8; basis_bytes];
        compile_basis_v3(
            BasisInputV3 {
                result_domain_id: result_domain.digest,
                ..provisional_basis_input
            },
            &mut output,
        )
        .map_err(|_| NarrowFixtureError::Basis)?;
        output
    };
    let linked_basis = finalized(
        input.registry_program,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        linked_basis_bytes,
    );

    let mut core_identity = MarketIdentity {
        market_id: identity([1; 32])?,
        realm_id: identity(input.realm_id)?,
        product_record: identity(product.digest)?,
        product_id: identity(product_id.to_bytes())?,
        resolution_policy: identity([0x51; 32])?,
        capability_manifest: identity([0x52; 32])?,
        selected_release_set: identity(input.release_set)?,
        registry_program: identity(input.registry_program.to_bytes())?,
        generation: input.generation,
    };
    let core_market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core_identity).as_slices(),
        &input.core_program,
    )
    .0;
    core_identity.market_id = identity(core_market.to_bytes())?;
    // An open Market carries no winner and no receipt; a resolved one must
    // carry both, and CoreState::encode refuses any other combination.
    let (phase, terminal_winner, terminal_receipt) = match input.terminal {
        None => (Phase::Open, 0, None),
        Some(terminal) => {
            if terminal.winner >= outcome_width {
                return Err(NarrowFixtureError::Width);
            }
            (
                Phase::Terminal,
                terminal.winner,
                Some(identity(terminal.receipt)?),
            )
        }
    };
    let core_state = CoreState {
        phase,
        readiness: Readiness::Consumed,
        terminal_winner,
        identity: core_identity,
        outstanding_capabilities: 1,
        principal_cap_sets: u64::MAX,
        rent_beneficiary: identity(input.rent_beneficiary.to_bytes())?,
        terminal_receipt,
        // What a founded Market's account actually holds. The Market and Realm
        // pair are not derivable here -- this fixture installs its Market
        // directly rather than founding one -- but the Product graph's four
        // record pairs are exactly the four `finalized` calls above, and Core's
        // founding records them. Leaving them zero would measure the search on
        // every instruction that reads this graph.
        bumps: StateBumpsV1 {
            product_graph: ProductGraphBumpsV1::record([
                product.raw_bump,
                product.staging_bump,
                result_domain.raw_bump,
                result_domain.staging_bump,
                portfolio.raw_bump,
                portfolio.staging_bump,
                linked_basis.raw_bump,
                linked_basis.staging_bump,
            ]),
            ..StateBumpsV1::UNRECORDED
        },
    }
    .encode()
    .map_err(|_| NarrowFixtureError::State)?;
    if core_state.len() != STATE_BYTES {
        return Err(NarrowFixtureError::State);
    }

    let claims_market = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, core_market.as_ref()],
        &input.claims_program,
    )
    .0;
    let mut supplies = vec![0_u64; input.outcome_count];
    *supplies
        .get_mut(input.funded_coordinate)
        .ok_or(NarrowFixtureError::State)? = input
        .funded_balance
        .checked_add(input.reserve_balance)
        .ok_or(NarrowFixtureError::State)?;
    let claims_market_bytes = encode_market(
        core_market,
        input,
        product_id.to_bytes(),
        semantic_basis_id,
        &supplies,
    )?;
    let mut actor_balances = vec![0_u64; input.outcome_count];
    *actor_balances
        .get_mut(input.funded_coordinate)
        .ok_or(NarrowFixtureError::State)? = input.funded_balance;
    let actor_position = position(
        input.claims_program,
        claims_market,
        input.actor_owner,
        semantic_basis_id,
        &actor_balances,
        input.position_revision,
    )?;
    let mut reserve_balances = vec![0_u64; input.outcome_count];
    *reserve_balances
        .get_mut(input.funded_coordinate)
        .ok_or(NarrowFixtureError::State)? = input.reserve_balance;
    let reserve_position = position(
        input.claims_program,
        claims_market,
        input.reserve_owner,
        semantic_basis_id,
        &reserve_balances,
        input.position_revision,
    )?;

    // One Claims representation root per Product result coordinate, weight 1.
    // The identity mapping is what makes the terminal payout legible: burning
    // at the winning coordinate redeems, burning anywhere else pays zero.
    let terms: Vec<[CompositionExposureTermV3; 1]> = (0..input.outcome_count)
        .map(|index| {
            [CompositionExposureTermV3 {
                product_coordinate: u32::try_from(index).unwrap_or_default(),
                numerator: 1,
            }]
        })
        .collect();
    let rows: Vec<CompositionExposureRowInputV3<'_>> = terms
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let mut node_id = [0x51_u8; 32];
            let index = u32::try_from(index).unwrap_or_default();
            node_id[0..4].copy_from_slice(&index.to_le_bytes());
            CompositionExposureRowInputV3 {
                node_id,
                denominator: 1,
                terms: row.as_slice(),
            }
        })
        .collect();
    let exposure_width = composition_exposure_bytes_v3(outcome_width, outcome_width)
        .map_err(|_| NarrowFixtureError::Exposure)?;
    let mut exposure_scratch = vec![0_u8; exposure_width];
    let mut exposure_bytes = vec![0_u8; exposure_width];
    encode_composition_exposure_v3_atomic(
        CompositionExposureInputV3 {
            market: core_market.to_bytes(),
            result_domain: result_domain.digest,
            release_set: input.release_set,
            product_basis: linked_basis.digest,
            representation_basis: semantic_basis_id,
            graph_id: input.graph_id,
            product_width: outcome_width,
            rows: &rows,
        },
        &mut exposure_scratch,
        &mut exposure_bytes,
    )
    .map_err(|_| NarrowFixtureError::Exposure)?;
    let exposure = finalized(
        input.registry_program,
        COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
        exposure_bytes,
    );

    Ok(NarrowFixtureV2 {
        exposure,
        exposure_id: input.exposure_id,
        outcome_count: u32::try_from(input.outcome_count).map_err(|_| NarrowFixtureError::State)?,
        product_id: product_id.to_bytes(),
        semantic_basis_id,
        payout_scale,
        product,
        result_domain,
        portfolio,
        linked_basis,
        core_market,
        core_state: core_state.to_vec(),
        claims_market,
        claims_market_bytes,
        actor_position,
        reserve_position,
    })
}

fn compile_product(
    registry: Pubkey,
    product_id: ContentId,
    semantic_basis_id: [u8; 32],
    cuts: &[i128],
    coefficients: &[u64],
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let mut product = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![
        0_u8;
        result_domain_record_bytes(cuts.len())
            .map_err(|_| NarrowFixtureError::Product)?
    ];
    let mut portfolio = vec![
        0_u8;
        portfolio_record_bytes(coefficients.len())
            .map_err(|_| NarrowFixtureError::Product)?
    ];
    compile_product_records_v2(
        registry,
        ProductCompilationInputV2 {
            product_id,
            coordinate_domain_id: content(COORDINATE_DOMAIN_ID)?,
            result_unit_id: content(RESULT_UNIT_ID)?,
            claim_basis_id: content([0x45; 32])?,
            liability_basis_id: content(semantic_basis_id)?,
            representation_release_id: content([0x46; 32])?,
            mapping_release_id: content([0x47; 32])?,
            cut_denominator: 1_000,
            cuts,
            portfolio_denominator: 1,
            coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .map_err(|_| NarrowFixtureError::Product)?;
    Ok((product, domain, portfolio))
}

fn finalized(owner: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> NarrowRecordV2 {
    let digest = hash(&bytes).to_bytes();
    let (raw, raw_bump) =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &owner);
    let (staging, staging_bump) =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &owner);
    NarrowRecordV2 {
        owner,
        schema,
        digest,
        raw,
        staging,
        raw_bump,
        staging_bump,
        bytes,
    }
}

fn semantic_basis_id_v3(bytes: &[u8]) -> Result<[u8; 32]> {
    let semantic = semantic_basis_preimage_v3(bytes).map_err(|_| NarrowFixtureError::Basis)?;
    Ok(hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes())
}

fn encode_market(
    core_market: Pubkey,
    input: NarrowFixtureInputV2,
    product_id: [u8; 32],
    semantic_basis_id: [u8; 32],
    supplies: &[u64],
) -> Result<Vec<u8>> {
    let width = CLAIMS_MARKET_HEADER_BYTES_V2
        .checked_add(
            supplies
                .len()
                .checked_mul(8)
                .ok_or(NarrowFixtureError::State)?,
        )
        .ok_or(NarrowFixtureError::State)?;
    let count = u32::try_from(supplies.len()).map_err(|_| NarrowFixtureError::State)?;
    let mut output = vec![0_u8; width];
    put(&mut output, 0, &CLAIMS_MARKET_MAGIC_V2)?;
    put(&mut output, 8, &CLAIMS_ABI_VERSION_V2.to_le_bytes())?;
    put(&mut output, 12, &count.to_le_bytes())?;
    put(&mut output, 16, &0_u64.to_le_bytes())?;
    for (offset, value) in [
        (24, core_market.to_bytes()),
        (56, input.release_set),
        (88, input.registry_program.to_bytes()),
        (120, product_id),
        (152, semantic_basis_id),
        (184, input.realm_id),
        (216, input.custody_context),
    ] {
        put(&mut output, offset, &value)?;
    }
    put(&mut output, 248, &input.generation.to_le_bytes())?;
    put_vector(&mut output, CLAIMS_MARKET_HEADER_BYTES_V2, supplies)?;
    Ok(output)
}

fn position(
    claims_program: Pubkey,
    claims_market: Pubkey,
    owner: Pubkey,
    semantic_basis_id: [u8; 32],
    balances: &[u64],
    revision: u64,
) -> Result<NarrowPositionV2> {
    let position_seeds = ProtocolPositionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
        .map_err(|_| NarrowFixtureError::Identity)?;
    let account = Pubkey::find_program_address(&position_seeds.as_slices(), &claims_program).0;
    let width = CLAIMS_POSITION_HEADER_BYTES_V2
        .checked_add(
            balances
                .len()
                .checked_mul(8)
                .ok_or(NarrowFixtureError::State)?,
        )
        .ok_or(NarrowFixtureError::State)?;
    let count = u32::try_from(balances.len()).map_err(|_| NarrowFixtureError::State)?;
    let mut bytes = vec![0_u8; width];
    put(&mut bytes, 0, &CLAIMS_POSITION_MAGIC_V2)?;
    put(&mut bytes, 8, &CLAIMS_ABI_VERSION_V2.to_le_bytes())?;
    put(&mut bytes, 12, &count.to_le_bytes())?;
    put(&mut bytes, 16, &revision.to_le_bytes())?;
    put(&mut bytes, 24, &claims_market.to_bytes())?;
    put(&mut bytes, 56, &owner.to_bytes())?;
    put(&mut bytes, 88, &semantic_basis_id)?;
    put_vector(&mut bytes, CLAIMS_POSITION_HEADER_BYTES_V2, balances)?;
    Ok(NarrowPositionV2 {
        owner,
        account,
        bytes,
    })
}

fn put_vector(output: &mut [u8], offset: usize, values: &[u64]) -> Result<()> {
    for (index, value) in values.iter().copied().enumerate() {
        let at = index
            .checked_mul(8)
            .and_then(|relative| offset.checked_add(relative))
            .ok_or(NarrowFixtureError::State)?;
        put(output, at, &value.to_le_bytes())?;
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(input.len())
        .ok_or(NarrowFixtureError::State)?;
    output
        .get_mut(offset..end)
        .ok_or(NarrowFixtureError::State)?
        .copy_from_slice(input);
    Ok(())
}

fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| NarrowFixtureError::Identity)
}

fn identity(bytes: [u8; 32]) -> Result<Identity> {
    Identity::new(bytes).map_err(|_| NarrowFixtureError::Identity)
}
