//! Chain-observation, K/N separation, hostile refusal, and packet corpus.

use dclutch_account_profile_contract::lifecycle_v3::{
    HEADER_BYTES as LIFECYCLE_POLICY_BYTES_V5, encode::encode_lifecycle_policy_v5_atomic,
};
use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
    HOT_MARKET_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3,
};
use dclutch_product_payoff_v2_codec::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, basis_record_bytes_v3,
        compile_basis_v3, semantic_basis_preimage_v3,
    },
};
use dclutch_product_runtime_v2::{ContentId, portfolio_record_bytes, result_domain_record_bytes};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product_runtime_v2_operator::{ProductCompilationInputV2, compile_product_records_v2};
use dclutch_rational_lifecycle_hot_v3::{
    CheckedRationalLifecycleHotOuterV3, RationalLifecycleHotStateV3,
    RationalLifecycleSelectedAccountProfileInputV5, RationalLifecycleSelectedBundleInputV5,
    RationalLifecycleSelectedBundleInputV6, RationalLifecycleSelectedSelectionV5,
    RationalLifecycleSelectedSelectionV6, build_rational_lifecycle_selected_bundle_v5,
    build_rational_lifecycle_selected_bundle_v6, lifecycle_logical_account_count_v3,
};
use dclutch_rational_representation_v2_contract::{
    AuthenticatedTokenBehaviorV2, TokenBehaviorRecordAdmissionV2, authenticate_token_behavior_v2,
};
use dclutch_rational_representation_v2_kernel::{
    DescriptorAdmissionV2, RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3, RepresentationDescriptorV2,
    descriptor_v3::{
        RepresentationDescriptorInputV3, encode_representation_descriptor_v3_atomic,
        representation_descriptor_bytes_v3,
    },
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LIFECYCLE_COMMON_ACCOUNT_COUNT_V2, LifecycleActionV2, LifecycleHeaderV2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_representation_composition_v3_kernel::{
    COMPOSITION_DESCRIPTOR_BYTES_V3, COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
    COMPOSITION_EXPOSURE_HEADER_BYTES_V3, COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
    COMPOSITION_GRAPH_HEADER_BYTES_V3, COMPOSITION_GRAPH_SCHEMA_ID_V3, COMPOSITION_NODE_BYTES_V3,
    COMPOSITION_TRANSLATION_SCHEMA_ID_V3, CanonicalTranslationInputV3,
    CompositionDescriptorInputV3, CompositionEdgeInputV3, CompositionExposureInputV3,
    CompositionExposureLayoutV3, CompositionExposureRowInputV3, CompositionExposureRowLayoutV3,
    CompositionExposureTermV3, CompositionGraphInputV3, CompositionNodeInputV3,
    CompositionNodeKindV3, DescriptorLayoutV3, EdgeLayoutV3, SparseTermV3,
    composition_exposure_bytes_v3, composition_graph_bytes_v3, composition_translation_bytes_v3,
    encode_canonical_translation_v3_atomic, encode_composition_descriptor_v3_atomic,
    encode_composition_exposure_v3_atomic, encode_composition_graph_v3_atomic,
};
use dclutch_representation_composition_v3_operator::{
    ClaimsLifecyclePlanV3, CompositionChainObservationV3, Error, FinalizedRecordObservationV3,
    ProductCompositionObservationV3, PublicationContextV3, PublicationTargetV3,
    RepresentationCompositionObservationV3, authenticate_composition_v3,
    build_claims_lifecycle_plan_v3, build_composition_admission_plan_v3, build_publication_plan_v3,
    compile_unsigned_packet_v0, hot_v3::build_composition_lifecycle_hot_plan_v3,
    hot_v6::build_composition_lifecycle_hot_plan_v6, validate_publication_candidates_v3,
};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_BYTES_V2,
    TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
};
use dclutch_versioned_message_operator::{Finality, Observation, ObservedAccount};
use solana_address_lookup_table_interface::{
    program as address_lookup_program,
    state::{AddressLookupTable, LookupTableMeta},
};
use solana_hash::Hash;
use solana_program::{
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;
use std::borrow::Cow;

const SLOT: u64 = 91;
const RENT: u64 = 1_000_000;

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn content(value: u8) -> ContentId {
    ContentId::new(id(value)).expect("nonzero fixture identity")
}

fn observation() -> Observation {
    Observation {
        slot: SLOT,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

#[derive(Clone)]
struct Record {
    schema: [u8; 32],
    raw: ObservedAccount,
    staging: ObservedAccount,
}

impl Record {
    fn new(registry: Pubkey, schema: [u8; 32], bytes: Vec<u8>) -> Self {
        let digest = hash(&bytes).to_bytes();
        let raw_key =
            Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
        let staging_key = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &registry,
        )
        .0;
        Self {
            schema,
            raw: ObservedAccount {
                observation: observation(),
                key: raw_key,
                owner: registry,
                lamports: RENT,
                executable: false,
                data: bytes,
            },
            staging: ObservedAccount {
                observation: observation(),
                key: staging_key,
                owner: system_program::ID,
                lamports: 9,
                executable: false,
                data: Vec::new(),
            },
        }
    }

    fn observed(&self) -> FinalizedRecordObservationV3<'_> {
        FinalizedRecordObservationV3 {
            schema_id: self.schema,
            raw: &self.raw,
            staging: &self.staging,
            raw_rent_minimum: 1,
        }
    }
}

struct Candidate {
    basis: Vec<u8>,
    descriptor: Vec<u8>,
    graph: Vec<u8>,
    translation: Vec<u8>,
    exposure: Vec<u8>,
    execution_descriptor: Vec<u8>,
}

fn categorical_basis(width: u32, product_id: [u8; 32], result_domain: [u8; 32]) -> Vec<u8> {
    let input = BasisInputV3 {
        kind: BasisKindV3::CategoricalQ1,
        product_id,
        result_domain_id: result_domain,
        coordinate_domain_id: id(3),
        result_unit_id: id(4),
        evaluator_release_id: id(5),
        basis_width: width,
        payout_scale: 1,
        knot_denominator: 1,
        knots: &[],
        terms: &[],
        failure_payouts: &[],
    };
    let mut bytes = vec![
        0;
        basis_record_bytes_v3(BasisKindV3::CategoricalQ1, width as usize, 0, 0)
            .expect("basis width")
    ];
    compile_basis_v3(input, &mut bytes).expect("categorical Product basis");
    bytes
}

fn semantic_basis(bytes: &[u8]) -> [u8; 32] {
    let semantic = semantic_basis_preimage_v3(bytes).expect("semantic preimage");
    hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes()
}

fn candidate(width: u32, product_id: [u8; 32], result_domain: [u8; 32]) -> Candidate {
    const K: u32 = 3;
    let basis = categorical_basis(width, product_id, result_domain);
    let native_basis = semantic_basis(&basis);
    let graph_id = id(30);
    let root_id = id(20);
    let nodes = [
        CompositionNodeInputV3 {
            id: id(10),
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            first_term: 0,
            term_count: 1,
            kind: CompositionNodeKindV3::Native,
            native_outcome: 0,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
        CompositionNodeInputV3 {
            id: id(11),
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            first_term: 1,
            term_count: 1,
            kind: CompositionNodeKindV3::Native,
            native_outcome: 1,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
        CompositionNodeInputV3 {
            id: id(12),
            rank: 0,
            first_edge: 0,
            edge_count: 0,
            first_term: 2,
            term_count: 1,
            kind: CompositionNodeKindV3::Native,
            native_outcome: 2,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
        CompositionNodeInputV3 {
            id: root_id,
            rank: 1,
            first_edge: 0,
            edge_count: 3,
            first_term: 3,
            term_count: 3,
            kind: CompositionNodeKindV3::Compose,
            native_outcome: 0,
            recipe_divisor: 1,
            flattened_denominator: 1,
        },
    ];
    let edges = [
        CompositionEdgeInputV3 {
            child_id: id(10),
            child_index: 0,
            coefficient: 1,
        },
        CompositionEdgeInputV3 {
            child_id: id(11),
            child_index: 1,
            coefficient: 1,
        },
        CompositionEdgeInputV3 {
            child_id: id(12),
            child_index: 2,
            coefficient: 1,
        },
    ];
    let terms = [
        SparseTermV3 {
            outcome: 0,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 1,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 2,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 0,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 1,
            numerator: 1,
        },
        SparseTermV3 {
            outcome: 2,
            numerator: 1,
        },
    ];
    let graph_len = composition_graph_bytes_v3(4, 3, 6).expect("graph bytes");
    let mut graph_scratch = vec![0; graph_len];
    let mut graph = vec![0; graph_len];
    encode_composition_graph_v3_atomic(
        CompositionGraphInputV3 {
            graph_id,
            root_id,
            outcome_count: K,
            nodes: &nodes,
            edges: &edges,
            terms: &terms,
        },
        &mut graph_scratch,
        &mut graph,
    )
    .expect("composition graph");
    let root_terms = &terms[3..];
    let translation_len = composition_translation_bytes_v3(3).expect("translation bytes");
    let mut translation_scratch = vec![0; translation_len];
    let mut translation = vec![0; translation_len];
    encode_canonical_translation_v3_atomic(
        CanonicalTranslationInputV3 {
            graph_id,
            root_id,
            outcome_count: K,
            denominator: 1,
            terms: root_terms,
        },
        &mut translation_scratch,
        &mut translation,
    )
    .expect("canonical translation");
    let mut descriptor_scratch = [0; COMPOSITION_DESCRIPTOR_BYTES_V3];
    let mut descriptor = [0; COMPOSITION_DESCRIPTOR_BYTES_V3];
    encode_composition_descriptor_v3_atomic(
        CompositionDescriptorInputV3 {
            market: id(40),
            result_domain,
            release_set: id(41),
            native_basis,
            graph_id,
            graph_digest: hash(&graph).to_bytes(),
            root_id,
            translation_id: id(31),
            translation_digest: hash(&translation).to_bytes(),
            outcome_count: K,
            node_count: 4,
            edge_count: 3,
            term_count: 6,
            root_denominator: 1,
        },
        &mut descriptor_scratch,
        &mut descriptor,
    )
    .expect("composition descriptor");
    let coordinates = if width == 1 {
        [0, 0, 0]
    } else {
        [0, width / 2, width - 1]
    };
    let row_terms = [
        [CompositionExposureTermV3 {
            product_coordinate: coordinates[0],
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: coordinates[1],
            numerator: 1,
        }],
        [CompositionExposureTermV3 {
            product_coordinate: coordinates[2],
            numerator: 1,
        }],
    ];
    let rows = [
        CompositionExposureRowInputV3 {
            node_id: id(10),
            denominator: 1,
            terms: &row_terms[0],
        },
        CompositionExposureRowInputV3 {
            node_id: id(11),
            denominator: 1,
            terms: &row_terms[1],
        },
        CompositionExposureRowInputV3 {
            node_id: id(12),
            denominator: 1,
            terms: &row_terms[2],
        },
    ];
    let exposure_len = composition_exposure_bytes_v3(K, K).expect("exposure bytes");
    let mut exposure_scratch = vec![0; exposure_len];
    let mut exposure = vec![0; exposure_len];
    encode_composition_exposure_v3_atomic(
        CompositionExposureInputV3 {
            market: id(40),
            result_domain,
            release_set: id(41),
            product_basis: hash(&basis).to_bytes(),
            representation_basis: native_basis,
            graph_id,
            product_width: width,
            rows: &rows,
        },
        &mut exposure_scratch,
        &mut exposure,
    )
    .expect("composition exposure");
    let execution_width = representation_descriptor_bytes_v3(K as usize)
        .expect("rational execution descriptor width");
    let mut execution_scratch = vec![0; execution_width];
    let mut execution_descriptor = vec![0; execution_width];
    encode_representation_descriptor_v3_atomic(
        RepresentationDescriptorInputV3 {
            exposure_id: hash(&exposure).to_bytes(),
            exposure_digest: hash(&exposure).to_bytes(),
            root_id,
            market: id(40),
            release_set: id(41),
            receipt_mint: id(75),
            token_program: TOKEN_2022_PROGRAM_ID,
            denominator: 1,
            coefficients: &[1, 1, 1],
        },
        &mut execution_scratch,
        &mut execution_descriptor,
    )
    .expect("rational execution descriptor");
    Candidate {
        basis,
        descriptor: descriptor.to_vec(),
        graph,
        translation,
        exposure,
        execution_descriptor,
    }
}

struct ChainFixture {
    registry: ObservedAccount,
    claims: ObservedAccount,
    product: Record,
    domain: Record,
    portfolio: Record,
    basis: Record,
    descriptor: Record,
    graph: Record,
    translation: Record,
    exposure: Record,
    execution_descriptor: Record,
}

impl ChainFixture {
    fn n258() -> Self {
        let registry_key = Pubkey::new_from_array(id(90));
        let cuts: Vec<i128> = (-128_i128..128).collect();
        let prebasis = categorical_basis(258, id(1), id(2));
        let liability_basis = semantic_basis(&prebasis);
        let coefficients = vec![1_u64; 258];
        let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
        let mut domain = vec![0; result_domain_record_bytes(cuts.len()).expect("domain width")];
        let mut portfolio =
            vec![0; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
        let compiled = compile_product_records_v2(
            registry_key,
            ProductCompilationInputV2 {
                product_id: content(1),
                coordinate_domain_id: content(3),
                result_unit_id: content(4),
                claim_basis_id: ContentId::new(liability_basis).expect("claim basis"),
                liability_basis_id: ContentId::new(liability_basis).expect("liability basis"),
                representation_release_id: content(6),
                mapping_release_id: content(7),
                cut_denominator: 1,
                cuts: &cuts,
                portfolio_denominator: 1,
                coefficients: &coefficients,
            },
            &mut product,
            &mut domain,
            &mut portfolio,
        )
        .expect("Product N258 graph");
        let result_domain = compiled.receipt.result_domain.content_digest.to_bytes();
        let candidate = candidate(258, id(1), result_domain);
        assert_eq!(semantic_basis(&candidate.basis), liability_basis);
        Self {
            registry: ObservedAccount {
                observation: observation(),
                key: registry_key,
                owner: Pubkey::new_from_array(id(91)),
                lamports: RENT,
                executable: true,
                data: Vec::new(),
            },
            claims: ObservedAccount {
                observation: observation(),
                key: Pubkey::new_from_array(id(92)),
                owner: Pubkey::new_from_array(id(93)),
                lamports: RENT,
                executable: true,
                data: Vec::new(),
            },
            product: Record::new(registry_key, PRODUCT_RECORD_SCHEMA_ID_V2, product.to_vec()),
            domain: Record::new(registry_key, RESULT_DOMAIN_SCHEMA_ID_V2, domain),
            portfolio: Record::new(registry_key, PORTFOLIO_SCHEMA_ID_V2, portfolio),
            basis: Record::new(
                registry_key,
                GRADED_BASIS_RECORD_SCHEMA_ID_V3,
                candidate.basis,
            ),
            descriptor: Record::new(
                registry_key,
                COMPOSITION_DESCRIPTOR_SCHEMA_ID_V3,
                candidate.descriptor,
            ),
            graph: Record::new(
                registry_key,
                COMPOSITION_GRAPH_SCHEMA_ID_V3,
                candidate.graph,
            ),
            translation: Record::new(
                registry_key,
                COMPOSITION_TRANSLATION_SCHEMA_ID_V3,
                candidate.translation,
            ),
            exposure: Record::new(
                registry_key,
                COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
                candidate.exposure,
            ),
            execution_descriptor: Record::new(
                registry_key,
                REPRESENTATION_DESCRIPTOR_SCHEMA_RELEASE_ID_V3,
                candidate.execution_descriptor,
            ),
        }
    }

    fn observed(&self) -> CompositionChainObservationV3<'_> {
        CompositionChainObservationV3 {
            registry_program: &self.registry,
            claims_program: &self.claims,
            product: ProductCompositionObservationV3 {
                product: self.product.observed(),
                result_domain: self.domain.observed(),
                portfolio: self.portfolio.observed(),
                product_basis: self.basis.observed(),
            },
            representation: RepresentationCompositionObservationV3 {
                execution_descriptor: self.execution_descriptor.observed(),
                descriptor: self.descriptor.observed(),
                graph: self.graph.observed(),
                translation: self.translation.observed(),
                exposure: self.exposure.observed(),
            },
        }
    }
}

fn selected_token_behavior(
    descriptor: dclutch_rational_representation_v2_kernel::RepresentationDescriptorV2<'_>,
) -> AuthenticatedTokenBehaviorV2 {
    let realm = id(94);
    let bytes = TokenBehaviorSelectionV2::new(realm, descriptor.release_set_id())
        .expect("Token behavior selection")
        .to_bytes();
    let digest = hash(&bytes).to_bytes();
    authenticate_token_behavior_v2(
        descriptor,
        realm,
        &bytes,
        TokenBehaviorRecordAdmissionV2 {
            selected_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            finalized_schema_id: TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            selected_content_digest: digest,
            finalized_content_digest: digest,
            recomputed_content_digest: digest,
            record_authenticated: true,
            market_realm_authenticated: true,
        },
    )
    .expect("authenticated Token behavior")
}

fn selected_lifecycle_bundle(
    fixture: &ChainFixture,
    admitted: dclutch_representation_composition_v3_operator::AdmittedCompositionV3<'_>,
    behavior: AuthenticatedTokenBehaviorV2,
) -> dclutch_rational_lifecycle_hot_v3::RationalLifecycleSelectedBundleV5 {
    let action = LifecycleActionV2::ActivateReceipt;
    let logical = usize::from(
        lifecycle_logical_account_count_v3(action, 0).expect("ActivateReceipt logical accounts"),
    );
    let mut lengths = vec![0_u32; logical];
    *lengths.get_mut(1).expect("Token selection coordinate") =
        u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("Token selection width");
    *lengths.get_mut(4).expect("ProductBasis coordinate") =
        u32::try_from(fixture.basis.raw.data.len()).expect("ProductBasis width");
    *lengths.get_mut(14).expect("descriptor coordinate") =
        u32::try_from(fixture.execution_descriptor.raw.data.len()).expect("descriptor width");
    let mut lifecycle_scratch = vec![0; LIFECYCLE_POLICY_BYTES_V5];
    let mut lifecycle = vec![0; LIFECYCLE_POLICY_BYTES_V5];
    encode_lifecycle_policy_v5_atomic(
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        &mut lifecycle_scratch,
        &mut lifecycle,
    )
    .expect("LifecycleV5");
    build_rational_lifecycle_selected_bundle_v5(RationalLifecycleSelectedBundleInputV5 {
        action,
        account_profile: RationalLifecycleSelectedAccountProfileInputV5 {
            logical_data_lengths: &lengths,
            product_basis: &fixture.basis.raw.data,
        },
        representation_descriptor: admitted.execution_descriptor(),
        authenticated_token_behavior: behavior,
        kind: id(95),
        root_schema: id(96),
        lifecycle_policy: &lifecycle,
        capacity_profile: id(97),
        root_state_bytes: 64,
    })
    .expect("selected lifecycle bundle")
}

fn selected_lifecycle_bundle_v6(
    fixture: &ChainFixture,
    behavior: AuthenticatedTokenBehaviorV2,
) -> dclutch_rational_lifecycle_hot_v3::RationalLifecycleSelectedBundleV6 {
    let action = LifecycleActionV2::ActivateReceipt;
    let logical = usize::from(
        lifecycle_logical_account_count_v3(action, 0).expect("ActivateReceipt logical accounts"),
    );
    let mut lengths = vec![0_u32; logical];
    *lengths.get_mut(1).expect("Token selection coordinate") =
        u32::try_from(TOKEN_BEHAVIOR_SELECTION_BYTES_V2).expect("Token selection width");
    *lengths.get_mut(4).expect("ProductBasis coordinate") =
        u32::try_from(fixture.basis.raw.data.len()).expect("ProductBasis width");
    *lengths.get_mut(14).expect("descriptor coordinate") =
        u32::try_from(fixture.execution_descriptor.raw.data.len()).expect("descriptor width");
    let mut lifecycle_scratch = vec![0; LIFECYCLE_POLICY_BYTES_V5];
    let mut lifecycle = vec![0; LIFECYCLE_POLICY_BYTES_V5];
    encode_lifecycle_policy_v5_atomic(
        &[],
        &[],
        &[],
        &[],
        &[],
        &[],
        &mut lifecycle_scratch,
        &mut lifecycle,
    )
    .expect("LifecycleV5");
    build_rational_lifecycle_selected_bundle_v6(RationalLifecycleSelectedBundleInputV6 {
        action,
        account_profile: RationalLifecycleSelectedAccountProfileInputV5 {
            logical_data_lengths: &lengths,
            product_basis: &fixture.basis.raw.data,
        },
        token_behavior_selection: behavior.selection(),
        kind: id(95),
        root_schema: id(96),
        lifecycle_policy: &lifecycle,
        capacity_profile: id(97),
        root_state_bytes: 64,
    })
    .expect("market-neutral V6 lifecycle bundle")
}

fn hot_fixed_accounts(
    market: Pubkey,
    trading: Pubkey,
    selected_config: Pubkey,
) -> Vec<AccountMeta> {
    let mut fixed: Vec<AccountMeta> = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
        .map(|_| AccountMeta::new_readonly(Pubkey::new_unique(), false))
        .collect();
    fixed
        .get_mut(HOT_ROOT_ACCOUNT_V3)
        .expect("root")
        .is_writable = true;
    fixed.get_mut(HOT_MARKET_ACCOUNT_V3).expect("Market").pubkey = market;
    fixed
        .get_mut(HOT_CONFIG_RAW_ACCOUNT_V3)
        .expect("selected config")
        .pubkey = selected_config;
    fixed
        .get_mut(HOT_TRADING_PROGRAM_ACCOUNT_V3)
        .expect("Trading")
        .pubkey = trading;
    fixed
        .get_mut(HOT_RENT_SYSVAR_ACCOUNT_V3)
        .expect("Rent")
        .pubkey = solana_sdk_ids::sysvar::rent::ID;
    fixed
        .get_mut(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .expect("Instructions")
        .pubkey = solana_sdk_ids::sysvar::instructions::ID;
    fixed
}

fn claims_activate_receipt_accounts(fixture: &ChainFixture, trading: Pubkey) -> Vec<AccountMeta> {
    let mut accounts: Vec<AccountMeta> = (0..LIFECYCLE_COMMON_ACCOUNT_COUNT_V2)
        .map(|_| AccountMeta::new_readonly(Pubkey::new_unique(), false))
        .collect();
    accounts.get_mut(0).expect("caller").is_signer = true;
    accounts.get_mut(1).expect("Trading").pubkey = trading;
    accounts.get_mut(3).expect("Claims").pubkey = fixture.claims.key;
    accounts.get_mut(9).expect("descriptor").pubkey = fixture.execution_descriptor.raw.key;
    accounts.get_mut(10).expect("descriptor cursor").pubkey =
        fixture.execution_descriptor.staging.key;
    accounts.get_mut(12).expect("receipt Mint").pubkey = Pubkey::new_from_array(id(75));
    accounts.get_mut(12).expect("receipt Mint").is_writable = true;
    accounts.get_mut(13).expect("Token program").pubkey =
        Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    accounts
}

fn observed_lookup_table(instruction: &Instruction, payer: Pubkey) -> ObservedAccount {
    let mut addresses = Vec::new();
    for meta in &instruction.accounts {
        if meta.pubkey != payer && !addresses.contains(&meta.pubkey) {
            addresses.push(meta.pubkey);
        }
    }
    let table = AddressLookupTable {
        meta: LookupTableMeta {
            authority: Some(Pubkey::new_from_array(id(98))),
            last_extended_slot: SLOT - 1,
            ..LookupTableMeta::default()
        },
        addresses: Cow::Owned(addresses),
    };
    ObservedAccount {
        observation: observation(),
        key: Pubkey::new_from_array(id(99)),
        owner: address_lookup_program::id(),
        lamports: RENT,
        executable: false,
        data: table.serialize_for_tests().expect("ALT bytes"),
    }
}

#[test]
fn k3_n1_publication_and_k3_n258_full_chain_admit() {
    let n1 = candidate(1, id(1), id(2));
    let (_, exposure) = validate_publication_candidates_v3(
        &n1.basis,
        &n1.descriptor,
        &n1.graph,
        &n1.translation,
        &n1.exposure,
    )
    .expect("K3/N1 exposure publication witness");
    assert_eq!(exposure.representation_width(), 3);
    assert_eq!(exposure.product_width(), 1);
    let mut scratch = [0; 3];
    let mut output = [0; 3];
    exposure
        .translate_product_payouts(&[7], &mut scratch, &mut output)
        .expect("K3/N1 exact translation");
    assert_eq!(output, [7, 7, 7]);

    let fixture = ChainFixture::n258();
    let plan =
        build_composition_admission_plan_v3(fixture.observed()).expect("full K3/N258 admission");
    assert_eq!(plan.representation_width(), 3);
    assert_eq!(plan.product_width(), 258);
    let admitted = plan.admitted();
    assert_eq!(admitted.representation_width(), 3);
    assert_eq!(admitted.product_width(), 258);
    assert_eq!(admitted.product().join.outcome_count, 258);
    let mut payouts = vec![0_u64; 258];
    *payouts.get_mut(0).expect("coordinate zero") = 3;
    *payouts.get_mut(129).expect("coordinate 129") = 5;
    *payouts.get_mut(257).expect("coordinate 257") = 8;
    admitted
        .exposure()
        .translate_product_payouts(&payouts, &mut scratch, &mut output)
        .expect("K3/N258 exact translation");
    assert_eq!(output, [3, 5, 8]);
}

#[test]
fn k3_n258_composition_admission_builds_packet_safe_selected_hot() {
    let fixture = ChainFixture::n258();
    let admission =
        build_composition_admission_plan_v3(fixture.observed()).expect("K3/N258 admission");
    let admitted = admission.admitted();
    let behavior = selected_token_behavior(admitted.execution_descriptor());
    let bundle = selected_lifecycle_bundle(&fixture, admitted, behavior);
    let trading = Pubkey::new_from_array(id(100));
    let fixed = hot_fixed_accounts(
        Pubkey::new_from_array(id(40)),
        trading,
        fixture.execution_descriptor.raw.key,
    );
    let root_data = [0_u8; 64];
    let state = RationalLifecycleHotStateV3 {
        fixed_accounts: &fixed,
        strategy_accounts: &[],
        root_data: &root_data,
        release_set: id(41),
        market: Pubkey::new_from_array(id(40)),
        generation: 1,
        finalized_slot: SLOT,
        hot_outer: Some(CheckedRationalLifecycleHotOuterV3 {
            trading_program: trading,
            artifact_release: id(101),
            checked_manifest_digest: id(102),
        }),
    };
    let claims_accounts = claims_activate_receipt_accounts(&fixture, trading);
    let descriptor = admitted.execution_descriptor();
    let header = LifecycleHeaderV2 {
        action: LifecycleActionV2::ActivateReceipt,
        release_set: descriptor.release_set_id(),
        market: descriptor.market_id(),
        graph_id: descriptor.graph_id(),
        descriptor_id: descriptor.descriptor_id(),
        parent_context: [0; 32],
        representation_authority: descriptor.representation_authority(),
        receipt_mint: descriptor.receipt_mint(),
        token_program: descriptor.token_program(),
        rent_credit: id(104),
        rent_program: id(105),
        generation: 1,
        expected_claims_market_revision: 2,
        observed_receipt_lamports: 100,
        receipt_rent_principal: 100,
        expected_receipt_supply: 0,
        outcome_count: descriptor.outcome_count(),
        coordinate_count: 0,
        rent_credit_before: 200,
        rent_credit_after: 200,
    };
    let plan = build_composition_lifecycle_hot_plan_v3(
        admission,
        &state,
        header,
        &[],
        fixture.claims.key,
        &claims_accounts,
        RationalLifecycleSelectedSelectionV5 {
            bundle: &bundle,
            authenticated_token_behavior: behavior,
        },
    )
    .expect("selected compact Hot");
    assert_eq!((plan.representation_width, plan.product_width), (3, 258));
    assert_eq!(plan.lifecycle.request.len(), 400);
    assert_eq!(plan.hot.instruction.data.len(), 528);
    assert!(plan.hot.required_wallet_signers.is_empty());
    let payer = Pubkey::new_from_array(id(106));
    let table = observed_lookup_table(&plan.hot.instruction, payer);
    let packet = compile_unsigned_packet_v0(
        payer,
        core::slice::from_ref(&plan.hot.instruction),
        Hash::new_from_array(id(107)),
        observation(),
        core::slice::from_ref(&table),
        1_400_000,
        1,
    )
    .expect("complete compact Hot packet");
    // 899 / 56 before the validated-artifact seal joined the fixed hot prefix.
    assert_eq!(packet.wire_bytes, 901);
    assert_eq!(packet.loaded_addresses, 56);
    assert_eq!(packet.required_signatures, 1);
    assert_eq!(1_232 - packet.wire_bytes, 331);

    assert_eq!(
        build_composition_lifecycle_hot_plan_v3(
            admission,
            &state,
            header,
            &[],
            Pubkey::new_from_array(id(108)),
            &claims_accounts,
            RationalLifecycleSelectedSelectionV5 {
                bundle: &bundle,
                authenticated_token_behavior: behavior,
            },
        )
        .err(),
        Some(Error::HotAdapter)
    );
    let stale_state = RationalLifecycleHotStateV3 {
        finalized_slot: SLOT + 1,
        ..state
    };
    assert_eq!(
        build_composition_lifecycle_hot_plan_v3(
            admission,
            &stale_state,
            header,
            &[],
            fixture.claims.key,
            &claims_accounts,
            RationalLifecycleSelectedSelectionV5 {
                bundle: &bundle,
                authenticated_token_behavior: behavior,
            },
        )
        .err(),
        Some(Error::HotAdapter)
    );
}

#[test]
fn k3_n258_v6_keeps_capability_market_neutral_and_binds_runtime_descriptor() {
    let fixture = ChainFixture::n258();
    let admission =
        build_composition_admission_plan_v3(fixture.observed()).expect("K3/N258 admission");
    let admitted = admission.admitted();
    let descriptor = admitted.execution_descriptor();
    let behavior = selected_token_behavior(descriptor);
    let bundle = selected_lifecycle_bundle_v6(&fixture, behavior);
    let trading = Pubkey::new_from_array(id(100));
    let fixed = hot_fixed_accounts(
        Pubkey::new_from_array(id(40)),
        trading,
        fixture.execution_descriptor.raw.key,
    );
    let root_data = [0_u8; 64];
    let state = RationalLifecycleHotStateV3 {
        fixed_accounts: &fixed,
        strategy_accounts: &[],
        root_data: &root_data,
        release_set: id(41),
        market: Pubkey::new_from_array(id(40)),
        generation: 1,
        finalized_slot: SLOT,
        hot_outer: Some(CheckedRationalLifecycleHotOuterV3 {
            trading_program: trading,
            artifact_release: id(101),
            checked_manifest_digest: id(102),
        }),
    };
    let claims_accounts = claims_activate_receipt_accounts(&fixture, trading);
    let header = LifecycleHeaderV2 {
        action: LifecycleActionV2::ActivateReceipt,
        release_set: descriptor.release_set_id(),
        market: descriptor.market_id(),
        graph_id: descriptor.graph_id(),
        descriptor_id: descriptor.descriptor_id(),
        parent_context: [0; 32],
        representation_authority: descriptor.representation_authority(),
        receipt_mint: descriptor.receipt_mint(),
        token_program: descriptor.token_program(),
        rent_credit: id(104),
        rent_program: id(105),
        generation: 1,
        expected_claims_market_revision: 2,
        observed_receipt_lamports: 100,
        receipt_rent_principal: 100,
        expected_receipt_supply: 0,
        outcome_count: descriptor.outcome_count(),
        coordinate_count: 0,
        rent_credit_before: 200,
        rent_credit_after: 200,
    };
    let plan = build_composition_lifecycle_hot_plan_v6(
        admission,
        &state,
        header,
        &[],
        fixture.claims.key,
        &claims_accounts,
        RationalLifecycleSelectedSelectionV6 {
            bundle: &bundle,
            authenticated_token_behavior: behavior,
            representation_descriptor: descriptor,
        },
    )
    .expect("market-neutral selected Hot");
    assert_eq!((plan.representation_width, plan.product_width), (3, 258));
    assert_eq!(plan.lifecycle.request.len(), 400);
    assert_eq!(plan.hot.instruction.data.len(), 528);
    let payer = Pubkey::new_from_array(id(106));
    let table = observed_lookup_table(&plan.hot.instruction, payer);
    let packet = compile_unsigned_packet_v0(
        payer,
        core::slice::from_ref(&plan.hot.instruction),
        Hash::new_from_array(id(107)),
        observation(),
        core::slice::from_ref(&table),
        1_400_000,
        1,
    )
    .expect("complete V6 compact Hot packet");
    // 899 / 56 before the validated-artifact seal joined the fixed hot prefix.
    assert_eq!(packet.wire_bytes, 901);
    assert_eq!(packet.loaded_addresses, 56);
    assert_eq!(packet.required_signatures, 1);
    assert_eq!(1_232 - packet.wire_bytes, 331);

    let hostile_id = id(109);
    let hostile_descriptor = RepresentationDescriptorV2::decode(
        &fixture.execution_descriptor.raw.data,
        DescriptorAdmissionV2 {
            selected_descriptor_id: hostile_id,
            finalized_descriptor_id: hostile_id,
            recomputed_descriptor_digest: hostile_id,
            finalized_descriptor_digest: hostile_id,
            record_authenticated: true,
            derived_representation_authority: descriptor.representation_authority(),
            authority_derivation_authenticated: true,
        },
    )
    .expect("hostile descriptor identity");
    assert_eq!(
        build_composition_lifecycle_hot_plan_v6(
            admission,
            &state,
            header,
            &[],
            fixture.claims.key,
            &claims_accounts,
            RationalLifecycleSelectedSelectionV6 {
                bundle: &bundle,
                authenticated_token_behavior: behavior,
                representation_descriptor: hostile_descriptor,
            },
        )
        .err(),
        Some(Error::HotAdapter)
    );
}

#[test]
fn substitution_cycle_rank_release_and_slot_refuse() {
    let canonical = candidate(1, id(1), id(2));
    let mut release = canonical.exposure.clone();
    *release
        .get_mut(CompositionExposureLayoutV3::RELEASE_SET)
        .expect("release byte") ^= 1;
    assert_eq!(
        validate_publication_candidates_v3(
            &canonical.basis,
            &canonical.descriptor,
            &canonical.graph,
            &canonical.translation,
            &release,
        )
        .err(),
        Some(Error::Composition)
    );

    let mut node_substitution = canonical.exposure.clone();
    node_substitution
        .get_mut(
            COMPOSITION_EXPOSURE_HEADER_BYTES_V3 + CompositionExposureRowLayoutV3::NODE_ID
                ..COMPOSITION_EXPOSURE_HEADER_BYTES_V3
                    + CompositionExposureRowLayoutV3::NODE_ID
                    + 32,
        )
        .expect("row node identity")
        .copy_from_slice(&id(99));
    assert_eq!(
        validate_publication_candidates_v3(
            &canonical.basis,
            &canonical.descriptor,
            &canonical.graph,
            &canonical.translation,
            &node_substitution,
        )
        .err(),
        Some(Error::Composition)
    );

    let mut rank = canonical.exposure.clone();
    let rank_offset = COMPOSITION_EXPOSURE_HEADER_BYTES_V3 + CompositionExposureRowLayoutV3::RANK;
    rank.get_mut(rank_offset..rank_offset + 4)
        .expect("rank bytes")
        .copy_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        validate_publication_candidates_v3(
            &canonical.basis,
            &canonical.descriptor,
            &canonical.graph,
            &canonical.translation,
            &rank,
        )
        .err(),
        Some(Error::Composition)
    );

    let mut cycle_graph = canonical.graph.clone();
    let first_edge = COMPOSITION_GRAPH_HEADER_BYTES_V3 + 4 * COMPOSITION_NODE_BYTES_V3;
    cycle_graph
        .get_mut(first_edge + EdgeLayoutV3::CHILD_INDEX..first_edge + EdgeLayoutV3::CHILD_INDEX + 4)
        .expect("child index")
        .copy_from_slice(&3_u32.to_le_bytes());
    let mut descriptor = canonical.descriptor.clone();
    descriptor
        .get_mut(DescriptorLayoutV3::GRAPH_DIGEST..DescriptorLayoutV3::GRAPH_DIGEST + 32)
        .expect("graph digest")
        .copy_from_slice(&hash(&cycle_graph).to_bytes());
    assert_eq!(
        validate_publication_candidates_v3(
            &canonical.basis,
            &descriptor,
            &cycle_graph,
            &canonical.translation,
            &canonical.exposure,
        )
        .err(),
        Some(Error::Composition)
    );

    let mut fixture = ChainFixture::n258();
    *fixture
        .exposure
        .raw
        .data
        .get_mut(0)
        .expect("exposure magic") ^= 1;
    assert_eq!(
        authenticate_composition_v3(fixture.observed()).err(),
        Some(Error::FinalizedRecord)
    );
    let mut fixture = ChainFixture::n258();
    fixture.graph.staging.observation.slot = SLOT + 1;
    assert_eq!(
        authenticate_composition_v3(fixture.observed()).err(),
        Some(Error::Observation)
    );
}

#[test]
fn publication_lifecycle_and_packet_geometry_are_exact() {
    let fixture = ChainFixture::n258();
    let admitted = authenticate_composition_v3(fixture.observed()).expect("composition");
    let publication = build_publication_plan_v3(
        PublicationContextV3 {
            record_program: fixture.registry.key,
            sponsor: Pubkey::new_from_array(id(70)),
            rent_credit: Pubkey::new_from_array(id(71)),
            current_slot: SLOT,
            cursor_rent_principal: 100,
        },
        PublicationTargetV3 {
            schema_id: COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
            bytes: &fixture.exposure.raw.data,
        },
    )
    .expect("canonical publication");
    assert_eq!(publication.instructions.len(), 3);
    assert_eq!(
        publication.content_digest,
        hash(&fixture.exposure.raw.data).to_bytes()
    );
    let mut publication_wire = Vec::new();
    for instruction in &publication.instructions {
        let packet = compile_unsigned_packet_v0(
            Pubkey::new_from_array(id(70)),
            core::slice::from_ref(instruction),
            Hash::new_from_array(id(72)),
            observation(),
            &[],
            400_000,
            1,
        )
        .expect("packet-safe publication step");
        assert!(packet.wire_bytes <= 1_232);
        assert_eq!(packet.required_signatures, 1);
        publication_wire.push(packet.wire_bytes);
    }
    assert_eq!(publication_wire, vec![599, 851, 338]);

    let lifecycle: ClaimsLifecyclePlanV3 = build_claims_lifecycle_plan_v3(
        admitted,
        LifecycleHeaderV2 {
            action: LifecycleActionV2::ActivateReceipt,
            release_set: id(41),
            market: id(40),
            graph_id: hash(&fixture.exposure.raw.data).to_bytes(),
            descriptor_id: hash(&fixture.execution_descriptor.raw.data).to_bytes(),
            parent_context: id(73),
            representation_authority: Pubkey::find_program_address(
                &[
                    RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
                    &hash(&fixture.execution_descriptor.raw.data).to_bytes(),
                ],
                &fixture.claims.key,
            )
            .0
            .to_bytes(),
            receipt_mint: id(75),
            token_program: TOKEN_2022_PROGRAM_ID,
            rent_credit: id(76),
            rent_program: id(77),
            generation: 1,
            expected_claims_market_revision: 2,
            observed_receipt_lamports: 100,
            receipt_rent_principal: 100,
            expected_receipt_supply: 0,
            outcome_count: 3,
            coordinate_count: 0,
            rent_credit_before: 200,
            rent_credit_after: 200,
        },
        &[],
    )
    .expect("canonical Claims lifecycle");
    assert_eq!(lifecycle.account_count, LIFECYCLE_COMMON_ACCOUNT_COUNT_V2);
    assert_eq!(lifecycle.request.len(), 400);
    let claims_accounts: Vec<AccountMeta> = (0..lifecycle.account_count)
        .map(|index| AccountMeta::new_readonly(Pubkey::new_unique(), index == 0))
        .collect();
    let claims = Instruction {
        program_id: Pubkey::new_from_array(id(78)),
        accounts: claims_accounts,
        data: lifecycle.request,
    };
    assert_eq!(
        compile_unsigned_packet_v0(
            Pubkey::new_from_array(id(70)),
            &[claims],
            Hash::new_from_array(id(72)),
            observation(),
            &[],
            500_000,
            1,
        )
        .err(),
        Some(Error::Packet)
    );

    let oversized = Instruction {
        program_id: Pubkey::new_from_array(id(79)),
        accounts: Vec::new(),
        data: vec![0; 1_232],
    };
    assert_eq!(
        compile_unsigned_packet_v0(
            Pubkey::new_from_array(id(70)),
            &[oversized],
            Hash::new_from_array(id(72)),
            observation(),
            &[],
            500_000,
            1,
        )
        .err(),
        Some(Error::Packet)
    );
}

#[test]
fn schema_slot_rank_and_publication_inputs_are_not_caller_fallbacks() {
    let mut fixture = ChainFixture::n258();
    fixture.descriptor.schema = COMPOSITION_GRAPH_SCHEMA_ID_V3;
    assert_eq!(
        authenticate_composition_v3(fixture.observed()).err(),
        Some(Error::FinalizedRecord)
    );
    let fixture = ChainFixture::n258();
    assert_eq!(
        build_publication_plan_v3(
            PublicationContextV3 {
                record_program: fixture.registry.key,
                sponsor: Pubkey::default(),
                rent_credit: Pubkey::new_from_array(id(71)),
                current_slot: SLOT,
                cursor_rent_principal: 100,
            },
            PublicationTargetV3 {
                schema_id: COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
                bytes: &fixture.exposure.raw.data,
            },
        )
        .err(),
        Some(Error::Publication)
    );
}
