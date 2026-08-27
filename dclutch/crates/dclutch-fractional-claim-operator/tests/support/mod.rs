#![allow(dead_code, missing_docs)]

use dclutch_account_profile_contract::{
    lifecycle_v3::SUCCESSOR_SCHEMA_RELEASE_ID, v2::SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID,
};
use dclutch_capability_program_contract::v3::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID;
use dclutch_effect_kernel::v3::SCHEMA_RELEASE_ID as EFFECT_PROGRAM_SCHEMA_ID;
use dclutch_execution_strategy_contract::v2::EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2;
use dclutch_fractional_claim_contract::{
    FractionalActionV1, FractionalRootInputV1, FractionalRootV1,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_TERMS_SCHEMA_ID_V1, FractionalTermsInputV1, OutcomeReserveV1,
    encode_fractional_terms_v1, fractional_terms_bytes_v1,
};
use dclutch_fractional_claim_operator::{
    CheckedFractionalReleaseInputV1, CheckedFractionalReleaseV1,
    FractionalArtifactRecordSnapshotV1, FractionalChainArtifactSnapshotV1,
    FractionalClaimsAccountRuleV1, FractionalPreparedChainArtifactsV1,
    build_fractional_finalized_artifact_bundle_v1, prepare_fractional_chain_artifacts_v1,
};
use dclutch_market_core_codec::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
};
use dclutch_product_runtime_v2::{
    ContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
    compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_request_profile_contract::SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID;
use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use dclutch_token_svm::{
    TOKEN_2022_PROGRAM_ID, TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, TokenBehaviorSelectionV2,
};
use dclutch_transition_vm::v3::SCHEMA_RELEASE_ID as TRANSITION_PROGRAM_SCHEMA_ID;
use sha2::{Digest, Sha256};
use solana_program::{
    account_info::AccountInfo, pubkey::Pubkey, rent::Rent, sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

#[derive(Clone)]
pub struct RecordPair {
    pub raw: ObservedAccount,
    pub staging: ObservedAccount,
}

impl RecordPair {
    fn snapshot(&self) -> FractionalArtifactRecordSnapshotV1<'_> {
        FractionalArtifactRecordSnapshotV1 {
            raw: &self.raw,
            staging: &self.staging,
        }
    }
}

/// Exact same-finalized account corpus for Fractional release tooling.
///
/// The supplied Claims rules are release-compiler inputs only. This fixture
/// intentionally does not define a Claims request, receipt, or postcondition.
#[derive(Clone)]
pub struct FractionalChainFixtureV1 {
    pub observation: Observation,
    pub payer: Pubkey,
    pub owner: Pubkey,
    pub checked: CheckedFractionalReleaseV1,
    pub registry_program: ObservedAccount,
    pub core_program: ObservedAccount,
    pub trading_program: ObservedAccount,
    pub claims_program: ObservedAccount,
    pub custody_program: ObservedAccount,
    pub token_program: ObservedAccount,
    pub rent_sysvar: ObservedAccount,
    pub market: ObservedAccount,
    pub product: RecordPair,
    pub result_domain: RecordPair,
    pub portfolio: RecordPair,
    pub descriptor: RecordPair,
    pub terms: RecordPair,
    pub token_behavior: RecordPair,
    pub account_profile: RecordPair,
    pub lifecycle: RecordPair,
    pub request: RecordPair,
    pub strategy: RecordPair,
    pub transition: RecordPair,
    pub effect: RecordPair,
    pub root: ObservedAccount,
    pub reserves: Vec<OutcomeReserveV1>,
}

impl FractionalChainFixtureV1 {
    pub fn new(
        action: FractionalActionV1,
        physical_profile: [u8; 32],
        claims_frame: &[FractionalClaimsAccountRuleV1],
    ) -> Self {
        Self::new_with_outcomes(action, physical_profile, claims_frame, 3)
    }

    pub fn new_with_outcomes(
        action: FractionalActionV1,
        physical_profile: [u8; 32],
        claims_frame: &[FractionalClaimsAccountRuleV1],
        outcome_count: usize,
    ) -> Self {
        assert!(
            outcome_count >= 2,
            "Product must include ordinary and failure outcomes"
        );
        let observation = Observation {
            slot: 9_001,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        };
        let rent = Rent::default();
        let registry = key(201);
        let core = key(202);
        let trading = key(203);
        let claims = key(204);
        let custody = key(205);
        let token = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
        let release_set = bytes(31);
        let realm = bytes(32);
        let product_id = content(33);
        let liability_basis = content(34);
        let representation_release = content(35);

        let ordinary_cuts = outcome_count
            .checked_sub(2)
            .expect("minimum Product width checked");
        let cuts: Vec<i128> = (0..ordinary_cuts)
            .map(|index| i128::try_from(index).expect("fixture cut index"))
            .collect();
        let mut domain = vec![0; result_domain_record_bytes(cuts.len()).expect("domain width")];
        compile_result_domain_v2(
            ResultDomainInputV2 {
                product_id,
                coordinate_domain_id: content(36),
                result_unit_id: content(37),
                liability_basis_id: liability_basis,
                representation_release_id: representation_release,
                mapping_release_id: content(38),
                cut_denominator: 1,
                cuts: &cuts,
            },
            &mut domain,
        )
        .expect("canonical Product domain");
        let domain_digest = digest(&domain);
        let coefficients = vec![1_u64; outcome_count];
        let mut portfolio =
            vec![0; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
        compile_portfolio_v2(
            PortfolioInputV2 {
                product_id,
                result_domain_id: content_bytes(domain_digest),
                claim_basis_id: content(39),
                liability_basis_id: liability_basis,
                representation_release_id: representation_release,
                denominator: 1,
                coefficients: &coefficients,
            },
            &mut portfolio,
        )
        .expect("canonical Product portfolio");
        let portfolio_digest = digest(&portfolio);
        let mut product = vec![0; PRODUCT_RECORD_BYTES_V2];
        ProductRecordV2::new(
            product_id,
            content_bytes(domain_digest),
            content_bytes(portfolio_digest),
        )
        .encode_into(&mut product)
        .expect("canonical Product root");
        let product_digest = digest(&product);

        let provisional_identity = MarketIdentity {
            market_id: identity(40),
            realm_id: identity_bytes(realm),
            product_record: identity_bytes(product_digest),
            product_id: identity_bytes(product_id.to_bytes()),
            resolution_policy: identity(41),
            capability_manifest: identity(42),
            selected_release_set: identity_bytes(release_set),
            registry_program: identity_bytes(registry.to_bytes()),
            generation: 7,
        };
        let market_key = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(provisional_identity).as_slices(),
            &core,
        )
        .0;
        let market_identity = MarketIdentity {
            market_id: identity_bytes(market_key.to_bytes()),
            ..provisional_identity
        };
        let rent_beneficiary = bytes(43);
        let market_data = CoreState {
            phase: Phase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity: market_identity,
            outstanding_capabilities: 1,
            rent_beneficiary: identity_bytes(rent_beneficiary),
            terminal_receipt: None,
        }
        .encode()
        .expect("canonical Open Market");

        let token_behavior = TokenBehaviorSelectionV2::new(realm, release_set)
            .expect("Token behavior")
            .to_bytes();
        let token_behavior_digest = digest(&token_behavior);
        let shard_mints: Vec<[u8; 32]> = (0..outcome_count)
            .map(|index| {
                let mut mint = [0_u8; 32];
                let ordinal = u32::try_from(index)
                    .expect("fixture outcome")
                    .checked_add(1)
                    .expect("fixture Mint ordinal");
                mint[..4].copy_from_slice(&ordinal.to_le_bytes());
                mint
            })
            .collect();
        let terms_width = fractional_terms_bytes_v1(shard_mints.len()).expect("terms width");
        let mut terms_scratch = vec![0; terms_width];
        let mut terms = vec![0; terms_width];
        encode_fractional_terms_v1(
            FractionalTermsInputV1 {
                market: market_key.to_bytes(),
                result_domain: domain_digest,
                release_set,
                token_program: TOKEN_2022_PROGRAM_ID,
                token_behavior: token_behavior_digest,
                denominator: 10,
                shard_mints: &shard_mints,
            },
            &mut terms_scratch,
            &mut terms,
        )
        .expect("canonical Fractional terms");
        let terms_digest = digest(&terms);

        let emitted =
            build_fractional_finalized_artifact_bundle_v1(action, physical_profile, claims_frame)
                .expect("canonical generic artifacts");
        let descriptor_digest = digest(&emitted.descriptor);
        let checked = CheckedFractionalReleaseV1::new(CheckedFractionalReleaseInputV1 {
            checked_manifest_digest: bytes(61),
            descriptor_id: descriptor_digest,
            terms_id: terms_digest,
            physical_profile,
            registry_program: registry,
            core_program: core,
            trading_program: trading,
            claims_program: claims,
            custody_program: custody,
            token_program: token,
            release_authenticated: true,
        })
        .expect("checked release projection");

        let (root_key, bump) = Pubkey::find_program_address(
            &[
                b"dclutch/fractional-root-v1",
                &terms_digest,
                market_key.as_ref(),
            ],
            &trading,
        );
        let root_principal = rent.minimum_balance(128);
        let root = FractionalRootV1::new(FractionalRootInputV1 {
            bump,
            terms: terms_digest,
            market: market_key.to_bytes(),
            rent_beneficiary,
            revision: 7,
            historical_rent_principal: root_principal,
        })
        .expect("canonical Fractional root")
        .to_bytes();

        Self {
            observation,
            payer: key(70),
            owner: key(70),
            checked,
            registry_program: program(observation, registry),
            core_program: program(observation, core),
            trading_program: program(observation, trading),
            claims_program: program(observation, claims),
            custody_program: program(observation, custody),
            token_program: program(observation, token),
            rent_sysvar: rent_account(observation, &rent),
            market: observed(
                observation,
                market_key,
                core,
                rent.minimum_balance(market_data.len()),
                false,
                market_data.to_vec(),
            ),
            product: record_pair(
                observation,
                registry,
                PRODUCT_RECORD_SCHEMA_ID_V2,
                product,
                &rent,
            ),
            result_domain: record_pair(
                observation,
                registry,
                RESULT_DOMAIN_SCHEMA_ID_V2,
                domain,
                &rent,
            ),
            portfolio: record_pair(
                observation,
                registry,
                PORTFOLIO_SCHEMA_ID_V2,
                portfolio,
                &rent,
            ),
            descriptor: record_pair(
                observation,
                registry,
                CAPABILITY_PROGRAM_SCHEMA_ID,
                emitted.descriptor.to_vec(),
                &rent,
            ),
            terms: record_pair(
                observation,
                registry,
                FRACTIONAL_TERMS_SCHEMA_ID_V1,
                terms,
                &rent,
            ),
            token_behavior: record_pair(
                observation,
                registry,
                TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                token_behavior.to_vec(),
                &rent,
            ),
            account_profile: record_pair(
                observation,
                registry,
                ACCOUNT_PROFILE_SCHEMA_ID,
                emitted.account_profile,
                &rent,
            ),
            lifecycle: record_pair(
                observation,
                registry,
                SUCCESSOR_SCHEMA_RELEASE_ID,
                emitted.lifecycle,
                &rent,
            ),
            request: record_pair(
                observation,
                registry,
                REQUEST_PROFILE_SCHEMA_ID,
                emitted.request_profile,
                &rent,
            ),
            strategy: record_pair(
                observation,
                registry,
                EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2,
                emitted.strategy.to_vec(),
                &rent,
            ),
            transition: record_pair(
                observation,
                registry,
                TRANSITION_PROGRAM_SCHEMA_ID,
                emitted.transition,
                &rent,
            ),
            effect: record_pair(
                observation,
                registry,
                EFFECT_PROGRAM_SCHEMA_ID,
                emitted.effect,
                &rent,
            ),
            root: observed(
                observation,
                root_key,
                trading,
                root_principal,
                false,
                root.to_vec(),
            ),
            reserves: (0..outcome_count)
                .map(|index| {
                    let locked_native_claims = u64::try_from(index)
                        .expect("fixture outcome")
                        .checked_add(2)
                        .expect("fixture reserve quantity");
                    OutcomeReserveV1 {
                        locked_native_claims,
                        shard_supply: locked_native_claims
                            .checked_mul(10)
                            .expect("fixture shard supply"),
                    }
                })
                .collect(),
        }
    }

    pub fn snapshot(&self) -> FractionalChainArtifactSnapshotV1<'_> {
        FractionalChainArtifactSnapshotV1 {
            registry_program: &self.registry_program,
            core_program: &self.core_program,
            trading_program: &self.trading_program,
            claims_program: &self.claims_program,
            custody_program: &self.custody_program,
            token_program: &self.token_program,
            rent_sysvar: &self.rent_sysvar,
            market: &self.market,
            product_raw: &self.product.raw,
            product_staging: &self.product.staging,
            result_domain_raw: &self.result_domain.raw,
            result_domain_staging: &self.result_domain.staging,
            portfolio_raw: &self.portfolio.raw,
            portfolio_staging: &self.portfolio.staging,
            descriptor: self.descriptor.snapshot(),
            terms: self.terms.snapshot(),
            token_behavior: self.token_behavior.snapshot(),
            account_profile: self.account_profile.snapshot(),
            lifecycle: self.lifecycle.snapshot(),
            request: self.request.snapshot(),
            strategy: self.strategy.snapshot(),
            transition: self.transition.snapshot(),
            effect: self.effect.snapshot(),
            root: &self.root,
        }
    }

    pub fn prepare(&self) -> FractionalPreparedChainArtifactsV1<'_> {
        prepare_fractional_chain_artifacts_v1(self.snapshot(), self.checked)
            .expect("same-finalized fixture authenticates")
    }
}

fn bytes(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn key(byte: u8) -> Pubkey {
    Pubkey::new_from_array(bytes(byte))
}

fn content(byte: u8) -> ContentId {
    content_bytes(bytes(byte))
}

fn content_bytes(value: [u8; 32]) -> ContentId {
    ContentId::new(value).expect("nonzero content identity")
}

fn identity(byte: u8) -> Identity {
    identity_bytes(bytes(byte))
}

fn identity_bytes(value: [u8; 32]) -> Identity {
    Identity::new(value).expect("nonzero Market identity")
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn observed(
    observation: Observation,
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    executable: bool,
    data: Vec<u8>,
) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        owner,
        lamports,
        executable,
        data,
    }
}

fn program(observation: Observation, key: Pubkey) -> ObservedAccount {
    observed(
        observation,
        key,
        bpf_loader_upgradeable::ID,
        1,
        true,
        Vec::new(),
    )
}

fn record_pair(
    observation: Observation,
    registry: Pubkey,
    schema: [u8; 32],
    data: Vec<u8>,
    rent: &Rent,
) -> RecordPair {
    let content_digest = digest(&data);
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &content_digest],
        &registry,
    )
    .0;
    let staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &content_digest],
        &registry,
    )
    .0;
    RecordPair {
        raw: observed(
            observation,
            raw,
            registry,
            rent.minimum_balance(data.len()),
            false,
            data,
        ),
        staging: observed(
            observation,
            staging,
            system_program::ID,
            7,
            false,
            Vec::new(),
        ),
    }
}

fn rent_account(observation: Observation, rent: &Rent) -> ObservedAccount {
    let mut data = vec![0; Rent::size_of()];
    let mut lamports = 1;
    let mut account = AccountInfo::new(
        &sysvar::rent::ID,
        false,
        false,
        &mut lamports,
        &mut data,
        &sysvar::ID,
        false,
    );
    rent.to_account_info(&mut account)
        .expect("serialize canonical Rent");
    drop(account);
    observed(
        observation,
        sysvar::rent::ID,
        sysvar::ID,
        lamports,
        false,
        data,
    )
}
