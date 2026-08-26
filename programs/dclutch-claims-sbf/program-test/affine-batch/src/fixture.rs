//! Shared 258-outcome Product Runtime V2/LBV2 fixture construction.
//!
//! This module is the sole fixture compiler used by affine and record-keyed
//! Position lifecycle ProgramTests. It returns canonical bytes and derived
//! account coordinates; each test still loads and observes real accounts.

use dclutch_claims_svm::protocol_position_v2::ProtocolPositionSeedsV2;
use dclutch_liability_basis_v2_kernel::product_claims::{
    CATEGORICAL_BASIS_BYTES_V2, CategoricalBasisInputV2, ContentIdV2,
    LINKED_CATEGORICAL_BASIS_BYTES_V2, encode_categorical_basis_v2, encode_linked_basis_record_v2,
};
use dclutch_market_core_codec::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness, STATE_BYTES,
};
use dclutch_product_runtime_v2::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{
    hash::{hash, hashv},
    pubkey::Pubkey,
};

const CLAIMS_MARKET_MAGIC_V2: [u8; 8] = *b"DCLLBM02";
const CLAIMS_POSITION_MAGIC_V2: [u8; 8] = *b"DCLLBP02";
const CLAIMS_ABI_VERSION_V2: u16 = 2;
const CLAIMS_MARKET_HEADER_BYTES_V2: usize = 256;
const CLAIMS_POSITION_HEADER_BYTES_V2: usize = 128;
const CLAIMS_MARKET_SEED_V2: &[u8] = b"dclutch:lbv2:market";
const LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2: [u8; 32] = [
    0x5c, 0x84, 0x2a, 0xe9, 0xe9, 0x15, 0x51, 0xd1, 0xaf, 0x99, 0xcf, 0x99, 0xfd, 0x53, 0x7f, 0x64,
    0xfb, 0x8d, 0xbf, 0x6a, 0x4e, 0x88, 0x3f, 0x22, 0xd9, 0x0b, 0xd5, 0xf3, 0x24, 0x5f, 0x6e, 0x2e,
];
const BASIS_SEMANTIC_ID_DOMAIN_V2: &[u8] = b"dclutch/lbv2/semantic-id/v2";
const BASIS_PRODUCT_LINK_OFFSET_V2: usize = 32;
const BASIS_PRODUCT_LINK_END_V2: usize = 64;
const OUTCOME_COUNT_V2: usize = 258;

/// Shared fixture compilation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureError {
    /// A nonzero identity refused construction.
    Identity,
    /// Runtime Product compilation or exact width refused.
    Product,
    /// Categorical or linked LBV2 encoding refused.
    Basis,
    /// Core or Claims fixed-layout encoding refused.
    State,
}

/// Result alias for shared fixture compilation.
pub type Result<T> = core::result::Result<T, FixtureError>;

/// Exact finalized-record fixture body and derived account keys.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedRecordFixtureV2 {
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
    /// Complete raw body.
    pub bytes: Vec<u8>,
}

/// One canonical LBV2 Position fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionFixtureV2 {
    /// Sole semantic Position owner.
    pub owner: Pubkey,
    /// Canonical Claims Position PDA.
    pub account: Pubkey,
    /// Complete runtime-width Position bytes.
    pub bytes: Vec<u8>,
}

/// Immutable inputs selecting one shared fixture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductLbv2FixtureInputV2 {
    /// Registry program owning Product Runtime V2 records.
    pub registry_program: Pubkey,
    /// Current Core program owning the linked LBV2 raw record and Market.
    pub core_program: Pubkey,
    /// Current Claims program owning aggregate and Positions.
    pub claims_program: Pubkey,
    /// Current execution release set.
    pub release_set: [u8; 32],
    /// Immutable Realm identity selected by Core/Claims.
    pub realm_id: [u8; 32],
    /// Immutable Custody replay namespace selected by Claims.
    pub custody_context: [u8; 32],
    /// Market generation.
    pub generation: u64,
    /// First Position owner, funded at the hostile coordinates.
    pub source_owner: Pubkey,
    /// Second Position owner, initially empty.
    pub destination_owner: Pubkey,
}

/// Complete canonical shared fixture and hostile substitution records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductLbv2FixtureV2 {
    /// Runtime outcome width, fixed to 258 for this evidence profile.
    pub outcome_count: u32,
    /// Stable semantic Product identity.
    pub product_id: [u8; 32],
    /// Semantic categorical-Q=1 liability basis identity.
    pub semantic_basis_id: [u8; 32],
    /// Canonical Product graph-root record.
    pub product: FinalizedRecordFixtureV2,
    /// Canonical Product-selected result-domain record.
    pub result_domain: FinalizedRecordFixtureV2,
    /// Canonical Product-selected portfolio record.
    pub portfolio: FinalizedRecordFixtureV2,
    /// Same-semantic-Product substitute graph root with changed portfolio.
    pub substituted_product: FinalizedRecordFixtureV2,
    /// Portfolio selected by the substituted Product graph root.
    pub substituted_portfolio: FinalizedRecordFixtureV2,
    /// Canonical Product-linked categorical LBV2 record.
    pub linked_basis: FinalizedRecordFixtureV2,
    /// Same semantic basis linked to another Product identity.
    pub substituted_linked_basis: FinalizedRecordFixtureV2,
    /// Canonical open Core Market PDA.
    pub core_market: Pubkey,
    /// Complete Core state bytes.
    pub core_state: Vec<u8>,
    /// Canonical LBV2 aggregate PDA.
    pub claims_market: Pubkey,
    /// Complete 258-outcome aggregate bytes.
    pub claims_market_bytes: Vec<u8>,
    /// Ordered source and destination LBV2 Position fixtures.
    pub positions: [PositionFixtureV2; 2],
}

/// Compile the single canonical Product Runtime V2/LBV2 fixture shared across
/// Claims affine and Position lifecycle ProgramTests.
pub fn compile_product_lbv2_fixture_v2(
    input: ProductLbv2FixtureInputV2,
) -> Result<ProductLbv2FixtureV2> {
    if input.release_set == [0; 32]
        || input.realm_id == [0; 32]
        || input.custody_context == [0; 32]
        || input.source_owner == input.destination_owner
    {
        return Err(FixtureError::Identity);
    }
    let product_id = content([0x41; 32])?;
    let other_product_id = content_id([0x42; 32])?;
    let mut embedded = [0_u8; CATEGORICAL_BASIS_BYTES_V2];
    encode_categorical_basis_v2(
        CategoricalBasisInputV2 {
            product_instance_id: content_id(product_id.to_bytes())?,
            claim_count: u32::try_from(OUTCOME_COUNT_V2).map_err(|_| FixtureError::Basis)?,
        },
        &mut embedded,
    )
    .map_err(|_| FixtureError::Basis)?;
    let semantic_basis_id = hashv(&[
        BASIS_SEMANTIC_ID_DOMAIN_V2,
        embedded
            .get(..BASIS_PRODUCT_LINK_OFFSET_V2)
            .ok_or(FixtureError::Basis)?,
        embedded
            .get(BASIS_PRODUCT_LINK_END_V2..)
            .ok_or(FixtureError::Basis)?,
    ])
    .to_bytes();

    let cuts: Vec<i128> = (0_i128..256_i128).collect();
    let coefficients = vec![1_u64; OUTCOME_COUNT_V2];
    let (product_bytes, domain_bytes, portfolio_bytes) = compile_product(
        input.registry_program,
        product_id,
        semantic_basis_id,
        &cuts,
        &coefficients,
    )?;
    let mut substituted_coefficients = coefficients;
    *substituted_coefficients
        .last_mut()
        .ok_or(FixtureError::Product)? = 2;
    let (substituted_product_bytes, substituted_domain_bytes, substituted_portfolio_bytes) =
        compile_product(
            input.registry_program,
            product_id,
            semantic_basis_id,
            &cuts,
            &substituted_coefficients,
        )?;
    if domain_bytes != substituted_domain_bytes {
        return Err(FixtureError::Product);
    }

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
    let substituted_product = finalized(
        input.registry_program,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        substituted_product_bytes,
    );
    let substituted_portfolio = finalized(
        input.registry_program,
        PORTFOLIO_SCHEMA_ID_V2,
        substituted_portfolio_bytes,
    );

    let linked_basis_bytes = linked_basis(
        content_id(product_id.to_bytes())?,
        content_id(semantic_basis_id)?,
        &embedded,
    )?;
    let mut substituted_embedded = [0_u8; CATEGORICAL_BASIS_BYTES_V2];
    encode_categorical_basis_v2(
        CategoricalBasisInputV2 {
            product_instance_id: other_product_id,
            claim_count: u32::try_from(OUTCOME_COUNT_V2).map_err(|_| FixtureError::Basis)?,
        },
        &mut substituted_embedded,
    )
    .map_err(|_| FixtureError::Basis)?;
    let substituted_semantic = hashv(&[
        BASIS_SEMANTIC_ID_DOMAIN_V2,
        substituted_embedded
            .get(..BASIS_PRODUCT_LINK_OFFSET_V2)
            .ok_or(FixtureError::Basis)?,
        substituted_embedded
            .get(BASIS_PRODUCT_LINK_END_V2..)
            .ok_or(FixtureError::Basis)?,
    ])
    .to_bytes();
    if substituted_semantic != semantic_basis_id {
        return Err(FixtureError::Basis);
    }
    let substituted_linked_bytes = linked_basis(
        other_product_id,
        content_id(semantic_basis_id)?,
        &substituted_embedded,
    )?;
    let linked_basis = finalized(
        input.core_program,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
        linked_basis_bytes,
    );
    let substituted_linked_basis = finalized(
        input.core_program,
        LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2,
        substituted_linked_bytes,
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
    let core_state = CoreState {
        phase: Phase::Open,
        readiness: Readiness::Consumed,
        terminal_winner: 0,
        identity: core_identity,
        outstanding_capabilities: 1,
        rent_beneficiary: identity(input.source_owner.to_bytes())?,
        terminal_receipt: None,
    }
    .encode()
    .map_err(|_| FixtureError::State)?;
    if core_state.len() != STATE_BYTES {
        return Err(FixtureError::State);
    }

    let claims_market = Pubkey::find_program_address(
        &[CLAIMS_MARKET_SEED_V2, core_market.as_ref()],
        &input.claims_program,
    )
    .0;
    let mut supplies = vec![0_u64; OUTCOME_COUNT_V2];
    *supplies.first_mut().ok_or(FixtureError::State)? = 7;
    *supplies.last_mut().ok_or(FixtureError::State)? = u64::MAX;
    let claims_market_bytes = encode_market(
        core_market,
        input,
        product_id.to_bytes(),
        semantic_basis_id,
        &supplies,
    )?;
    let source_balances = supplies;
    let destination_balances = vec![0_u64; OUTCOME_COUNT_V2];
    let positions = [
        position(
            input.claims_program,
            claims_market,
            input.source_owner,
            semantic_basis_id,
            &source_balances,
        )?,
        position(
            input.claims_program,
            claims_market,
            input.destination_owner,
            semantic_basis_id,
            &destination_balances,
        )?,
    ];
    Ok(ProductLbv2FixtureV2 {
        outcome_count: u32::try_from(OUTCOME_COUNT_V2).map_err(|_| FixtureError::State)?,
        product_id: product_id.to_bytes(),
        semantic_basis_id,
        product,
        result_domain,
        portfolio,
        substituted_product,
        substituted_portfolio,
        linked_basis,
        substituted_linked_basis,
        core_market,
        core_state: core_state.to_vec(),
        claims_market,
        claims_market_bytes,
        positions,
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
    let mut domain =
        vec![0_u8; result_domain_record_bytes(cuts.len()).map_err(|_| FixtureError::Product)?];
    let mut portfolio =
        vec![0_u8; portfolio_record_bytes(coefficients.len()).map_err(|_| FixtureError::Product)?];
    compile_product_records_v2(
        registry,
        ProductCompilationInputV2 {
            product_id,
            coordinate_domain_id: content([0x43; 32])?,
            result_unit_id: content([0x44; 32])?,
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
    .map_err(|_| FixtureError::Product)?;
    Ok((product, domain, portfolio))
}

fn finalized(owner: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> FinalizedRecordFixtureV2 {
    let digest = hash(&bytes).to_bytes();
    let raw = Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &owner).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &owner).0;
    FinalizedRecordFixtureV2 {
        owner,
        schema,
        digest,
        raw,
        staging,
        bytes,
    }
}

fn linked_basis(
    product_id: ContentIdV2,
    semantic_basis_id: ContentIdV2,
    embedded: &[u8],
) -> Result<Vec<u8>> {
    let mut output = vec![0_u8; LINKED_CATEGORICAL_BASIS_BYTES_V2];
    encode_linked_basis_record_v2(product_id, semantic_basis_id, embedded, &mut output)
        .map_err(|_| FixtureError::Basis)?;
    Ok(output)
}

fn encode_market(
    core_market: Pubkey,
    input: ProductLbv2FixtureInputV2,
    product_id: [u8; 32],
    semantic_basis_id: [u8; 32],
    supplies: &[u64],
) -> Result<Vec<u8>> {
    let width = CLAIMS_MARKET_HEADER_BYTES_V2
        .checked_add(supplies.len().checked_mul(8).ok_or(FixtureError::State)?)
        .ok_or(FixtureError::State)?;
    let count = u32::try_from(supplies.len()).map_err(|_| FixtureError::State)?;
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
) -> Result<PositionFixtureV2> {
    let position_seeds = ProtocolPositionSeedsV2::new(claims_market.to_bytes(), owner.to_bytes())
        .map_err(|_| FixtureError::Identity)?;
    let account = Pubkey::find_program_address(&position_seeds.as_slices(), &claims_program).0;
    let width = CLAIMS_POSITION_HEADER_BYTES_V2
        .checked_add(balances.len().checked_mul(8).ok_or(FixtureError::State)?)
        .ok_or(FixtureError::State)?;
    let count = u32::try_from(balances.len()).map_err(|_| FixtureError::State)?;
    let mut bytes = vec![0_u8; width];
    put(&mut bytes, 0, &CLAIMS_POSITION_MAGIC_V2)?;
    put(&mut bytes, 8, &CLAIMS_ABI_VERSION_V2.to_le_bytes())?;
    put(&mut bytes, 12, &count.to_le_bytes())?;
    put(&mut bytes, 16, &0_u64.to_le_bytes())?;
    put(&mut bytes, 24, &claims_market.to_bytes())?;
    put(&mut bytes, 56, &owner.to_bytes())?;
    put(&mut bytes, 88, &semantic_basis_id)?;
    put_vector(&mut bytes, CLAIMS_POSITION_HEADER_BYTES_V2, balances)?;
    Ok(PositionFixtureV2 {
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
            .ok_or(FixtureError::State)?;
        put(output, at, &value.to_le_bytes())?;
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, input: &[u8]) -> Result<()> {
    let end = offset.checked_add(input.len()).ok_or(FixtureError::State)?;
    output
        .get_mut(offset..end)
        .ok_or(FixtureError::State)?
        .copy_from_slice(input);
    Ok(())
}

fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| FixtureError::Identity)
}

fn content_id(bytes: [u8; 32]) -> Result<ContentIdV2> {
    ContentIdV2::new(bytes).map_err(|_| FixtureError::Identity)
}

fn identity(bytes: [u8; 32]) -> Result<Identity> {
    Identity::new(bytes).map_err(|_| FixtureError::Identity)
}
