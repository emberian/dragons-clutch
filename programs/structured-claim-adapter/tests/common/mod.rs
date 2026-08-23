#![allow(dead_code)]

use clutch_kernel::{BasisMode, MarketState, PayoutSet, PayoutVector, MAX_PAYOUTS};
use clutch_solana_layout::{
    canonical_outcome_id, portfolio_settlement::NativePortfolioClaimV1, Hash32, HoardAccount,
    MarketAccount, PayoutVectorBytes, PositionAccount, SupplyLedgerAccount, TermsAccount,
    MAX_KNOTS, MAX_OUTCOMES, PAYOUT_MAP_UNUSED,
};
use clutch_structured_claim::DeploymentBinding;
use clutch_structured_claim_adapter::{
    canonical_replay_namespace, canonical_wrapper_product_id, AccountAccess, AccountRole,
    AccountSet, Action, AdapterContext, AddressBinding, AuthenticatedMarket, BaseReplayProjection,
    ExpectedPostState, Key, MintProjection, PdaVerifier, RequestV1, RuntimeDeployments,
    StructuredClaimDescriptorV1, TokenAccountProjection, WrapperReplayV1, DESCRIPTOR_SEED,
    MAX_CPI_STEPS, MINT_SEED, REPLAY_SEED, VAULT_OWNER_SEED,
};
use sha2::{Digest, Sha256};

pub fn key(marker: u8) -> Key {
    [marker; 32]
}

fn hash(marker: u8) -> Hash32 {
    Hash32::from_bytes(key(marker))
}

pub fn terms(cap: u64) -> TermsAccount {
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut left = [0; MAX_OUTCOMES];
    left[0] = 8;
    let mut right = [0; MAX_OUTCOMES];
    right[1] = 8;
    payouts[0] = PayoutVectorBytes {
        denominator: 8,
        weights: left,
    };
    payouts[1] = PayoutVectorBytes {
        denominator: 8,
        weights: right,
    };
    let mut knots = [0; MAX_KNOTS];
    knots[0] = 100;
    knots[1] = 116;
    let mut value = TermsAccount {
        terms: Hash32::ZERO,
        realm: hash(2),
        profile: hash(3),
        feed: hash(4),
        price_grid: hash(5),
        outcome_count: 2,
        payout_count: 2,
        payouts,
        grid_family_id: 1,
        grid_version: 1,
        bucket_seconds: 60,
        expected_start_bucket: 100,
        expected_end_bucket_exclusive: 110,
        maturity_horizon_buckets: 10,
        coverage_policy_id: 1,
        repair_policy_id: 1,
        failure_policy_id: 1,
        statistic_id: 1,
        ambiguity_policy_id: 1,
        edge_policy_id: 1,
        basis_degree: 1,
        knot_count: 2,
        uniform_log2_spacing: 4,
        failure_payout_index: 0,
        coverage_policy_parameter: 0,
        repair_generation: 1,
        source_version: 1,
        evaluator_version: 1,
        source_adapter_id: hash(6),
        payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
        knots,
        collateral_cap: cap,
        stored_bump: 7,
        flags: 0,
    };
    value.terms = value.recomputed_terms_digest().unwrap();
    value.validate().unwrap();
    value
}

fn deployment() -> DeploymentBinding {
    DeploymentBinding {
        wrapper_program: key(11),
        wrapper_program_data: key(12),
        wrapper_deployment_slot: 1_000,
        base_program: key(13),
        base_program_data: key(14),
        base_deployment_slot: 2_000,
        token_2022_program: key(15),
        token_2022_program_data: key(16),
        token_2022_deployment_slot: 3_000,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FakePda;

impl PdaVerifier for FakePda {
    fn verify(
        &self,
        program: &Key,
        address: &Key,
        prefix: &[u8],
        product_id: &Key,
        bump: u8,
    ) -> bool {
        *address == fake_pda(*program, prefix, *product_id, bump)
    }
}

pub fn fake_pda(program: Key, prefix: &[u8], product: Key, bump: u8) -> Key {
    let mut hasher = Sha256::new();
    hasher.update(program);
    hasher.update(prefix);
    hasher.update(product);
    hasher.update([bump]);
    hasher.finalize().into()
}

#[derive(Clone, Debug)]
pub struct Fixture {
    pub descriptor: StructuredClaimDescriptorV1,
    pub deployments: RuntimeDeployments,
    pub addresses: AddressBinding,
    pub native_claim_id: Key,
    pub product_id: Key,
    pub market: MarketAccount,
    pub terms: TermsAccount,
    pub hoard: HoardAccount,
    pub supply: SupplyLedgerAccount,
    pub base: MarketState,
    pub vault: PositionAccount,
    pub source: PositionAccount,
    pub mint: MintProjection,
    pub token: TokenAccountProjection,
    pub wrapper_replay: WrapperReplayV1,
    pub source_replay: BaseReplayProjection,
    pub vault_replay: BaseReplayProjection,
    pub accounts: AccountSet,
    pub request: RequestV1,
}

impl Fixture {
    pub fn new() -> Self {
        let terms = terms(1_000);
        let market_id = hash(1);
        let mut outcomes = [Hash32::ZERO; MAX_OUTCOMES];
        outcomes[0] = canonical_outcome_id(market_id, 0);
        outcomes[1] = canonical_outcome_id(market_id, 1);
        let market = MarketAccount {
            market: market_id,
            realm: terms.realm,
            profile: terms.profile,
            terms: terms.terms,
            outcome_count: 2,
            lifecycle: 0,
            stored_bump: 1,
            hoard_bump: 2,
            outcomes,
            feed: terms.feed,
            collateral_cap: terms.collateral_cap,
            created_slot: 1,
            reserved: Hash32::ZERO,
        };
        market.validate().unwrap();

        let mut vectors = [PayoutVector::ZERO; MAX_PAYOUTS];
        vectors[0] = PayoutVector::new(8, terms.payouts[0].weights);
        vectors[1] = PayoutVector::new(8, terms.payouts[1].weights);
        let payouts = PayoutSet::new(2, 2, vectors);
        let mut base = MarketState::new(2, BasisMode::DerivedBasis, payouts, 100).unwrap();
        base.total_supply[0] = 100;
        base.total_supply[1] = 100;
        base.check_invariants().unwrap();
        let hoard = HoardAccount {
            market: market_id,
            realm: terms.realm,
            authority: hash(20),
            collateral_atoms: 100,
            stored_bump: 2,
            flags: 0,
        };
        let mut internal_supply = [0; MAX_OUTCOMES];
        internal_supply[0] = 100;
        internal_supply[1] = 100;
        let supply = SupplyLedgerAccount {
            market: market_id,
            realm: terms.realm,
            generation: 1,
            outcome_count: 2,
            internal_supply,
            external_supply: [0; MAX_OUTCOMES],
            stored_bump: 3,
            flags: 0,
        };
        let source_owner = key(40);
        let mut source_internal = [0; MAX_OUTCOMES];
        source_internal[0] = 10;
        source_internal[1] = 20;
        let source = PositionAccount {
            market: market_id,
            owner: Hash32::from_bytes(source_owner),
            generation: 7,
            internal: source_internal,
            cash_atoms: 10,
            reserved_cash_atoms: 2,
            stored_bump: 4,
            close_state: 0,
        };

        let deployment = deployment();
        let deployments = RuntimeDeployments {
            binding: deployment,
            upgradeable_loader: key(17),
            program_owners: [key(17); 3],
            program_data_owners: [key(17); 3],
            linked_program_data: [
                deployment.wrapper_program_data,
                deployment.base_program_data,
                deployment.token_2022_program_data,
            ],
            executable_mask: 0b111,
        };
        let mut primitive = [0; MAX_OUTCOMES];
        primitive[0] = 1;
        primitive[1] = 2;
        let descriptor = StructuredClaimDescriptorV1 {
            base_program: deployment.base_program,
            base_program_data: deployment.base_program_data,
            base_deployment_slot: deployment.base_deployment_slot,
            wrapper_program_data: deployment.wrapper_program_data,
            wrapper_deployment_slot: deployment.wrapper_deployment_slot,
            token_2022_program: deployment.token_2022_program,
            token_2022_program_data: deployment.token_2022_program_data,
            token_2022_deployment_slot: deployment.token_2022_deployment_slot,
            market: market_id.bytes(),
            terms: terms.terms.bytes(),
            primitive,
            state: 0,
            descriptor_bump: 201,
            mint_bump: 202,
            vault_owner_bump: 203,
        };
        let (native, gcd) = NativePortfolioClaimV1::compile(market_id, &terms, primitive).unwrap();
        assert_eq!(gcd, 1);
        let native_claim_id = native.claim.bytes();
        let product_id = canonical_wrapper_product_id(&deployment, native_claim_id).unwrap();
        let addresses = AddressBinding {
            descriptor: fake_pda(
                deployment.wrapper_program,
                DESCRIPTOR_SEED,
                product_id,
                descriptor.descriptor_bump,
            ),
            mint: fake_pda(
                deployment.wrapper_program,
                MINT_SEED,
                product_id,
                descriptor.mint_bump,
            ),
            vault_owner: fake_pda(
                deployment.wrapper_program,
                VAULT_OWNER_SEED,
                product_id,
                descriptor.vault_owner_bump,
            ),
        };
        let vault = PositionAccount {
            market: market_id,
            owner: Hash32::from_bytes(addresses.vault_owner),
            generation: 5,
            internal: [0; MAX_OUTCOMES],
            cash_atoms: 0,
            reserved_cash_atoms: 0,
            stored_bump: 5,
            close_state: 0,
        };
        let mint = MintProjection {
            key: addresses.mint,
            token_program: deployment.token_2022_program,
            supply: 0,
            mint_authority: addresses.vault_owner,
            initialized: true,
            decimals: 0,
            freeze_authority_present: false,
            extension_mask: 0,
        };
        let token = TokenAccountProjection {
            key: key(41),
            token_program: deployment.token_2022_program,
            mint: addresses.mint,
            authority: source_owner,
            amount: 0,
            initialized: true,
            frozen: false,
            native: false,
            delegate_present: false,
            close_authority_present: false,
            extension_mask: 1,
        };
        let wrapper_replay = WrapperReplayV1 {
            descriptor: addresses.descriptor,
            actor: source_owner,
            sequence: 3,
            stored_bump: 6,
        };
        let source_replay = BaseReplayProjection {
            key: key(42),
            market: market_id.bytes(),
            owner: source_owner,
            position_generation: source.generation,
            sequence: 5,
            stored_bump: 7,
            flags: 0,
        };
        let vault_replay = BaseReplayProjection {
            key: key(43),
            market: market_id.bytes(),
            owner: addresses.vault_owner,
            position_generation: vault.generation,
            sequence: 7,
            stored_bump: 8,
            flags: 0,
        };
        let mut fixture = Self {
            descriptor,
            deployments,
            addresses,
            native_claim_id,
            product_id,
            market,
            terms,
            hoard,
            supply,
            base,
            vault,
            source,
            mint,
            token,
            wrapper_replay,
            source_replay,
            vault_replay,
            accounts: AccountSet {
                count: 0,
                accounts: [AccountAccess::EMPTY; 17],
            },
            request: RequestV1 {
                action: Action::WrapCanonical,
                wrapper_sequence: 3,
                source_base_sequence: 5,
                vault_base_sequence: 7,
                quantity: 2,
                expected_mint_supply: 0,
                expected_holder_amount: 0,
                source_generation: 7,
                vault_generation: 5,
            },
        };
        fixture.accounts = fixture.full_accounts();
        fixture
    }

    pub fn context(&self) -> AdapterContext<'_> {
        AdapterContext {
            descriptor: &self.descriptor,
            native_claim_id: self.native_claim_id,
            product_id: self.product_id,
            deployments: &self.deployments,
            addresses: &self.addresses,
            market: AuthenticatedMarket {
                market: &self.market,
                terms: &self.terms,
                hoard: &self.hoard,
                supply: &self.supply,
                base: &self.base,
            },
            vault_position: &self.vault,
            mint: &self.mint,
            holder_position: Some(&self.source),
            holder_token: Some(&self.token),
            wrapper_replay: &self.wrapper_replay,
            source_replay: Some(&self.source_replay),
            vault_replay: &self.vault_replay,
            accounts: &self.accounts,
        }
    }

    pub fn compact_context(&self) -> AdapterContext<'_> {
        AdapterContext {
            holder_position: None,
            holder_token: None,
            source_replay: None,
            ..self.context()
        }
    }

    pub fn full_accounts(&self) -> AccountSet {
        let wrapper = self.deployments.binding.wrapper_program;
        let base = self.deployments.binding.base_program;
        let token = self.deployments.binding.token_2022_program;
        let loader = self.deployments.upgradeable_loader;
        let mut accounts = [AccountAccess::EMPTY; 17];
        let values = [
            access(
                AccountRole::WrapperProgram,
                wrapper,
                loader,
                false,
                false,
                true,
            ),
            access(AccountRole::BaseProgram, base, loader, false, false, true),
            access(AccountRole::TokenProgram, token, loader, false, false, true),
            access(
                AccountRole::Descriptor,
                self.addresses.descriptor,
                wrapper,
                false,
                false,
                false,
            ),
            access(
                AccountRole::Mint,
                self.addresses.mint,
                token,
                false,
                true,
                false,
            ),
            access(
                AccountRole::VaultPosition,
                key(50),
                base,
                false,
                true,
                false,
            ),
            access(
                AccountRole::HolderPosition,
                key(51),
                base,
                false,
                true,
                false,
            ),
            access(
                AccountRole::HolderToken,
                self.token.key,
                token,
                false,
                true,
                false,
            ),
            access(
                AccountRole::WrapperReplay,
                fake_pda(
                    wrapper,
                    REPLAY_SEED,
                    canonical_replay_namespace(self.product_id, self.wrapper_replay.actor).unwrap(),
                    self.wrapper_replay.stored_bump,
                ),
                wrapper,
                false,
                true,
                false,
            ),
            access(
                AccountRole::SourceReplay,
                self.source_replay.key,
                base,
                false,
                true,
                false,
            ),
            access(
                AccountRole::VaultReplay,
                self.vault_replay.key,
                base,
                false,
                true,
                false,
            ),
            access(AccountRole::Market, key(53), base, false, false, false),
            access(AccountRole::Terms, key(54), base, false, false, false),
            access(AccountRole::Hoard, key(55), base, false, true, false),
            access(AccountRole::SupplyLedger, key(56), base, false, true, false),
            access(AccountRole::Kernel, key(57), base, false, true, false),
            access(
                AccountRole::Actor,
                self.wrapper_replay.actor,
                key(99),
                true,
                false,
                false,
            ),
        ];
        accounts.copy_from_slice(&values);
        AccountSet {
            count: 17,
            accounts,
        }
    }

    pub fn compact_accounts(&self) -> AccountSet {
        let full = self.full_accounts();
        let mut accounts = [AccountAccess::EMPTY; 17];
        let mut count = 0;
        for account in full.accounts {
            if !matches!(
                account.role,
                AccountRole::HolderPosition | AccountRole::HolderToken | AccountRole::SourceReplay
            ) {
                accounts[count] = account;
                count += 1;
            }
        }
        AccountSet {
            count: count as u8,
            accounts,
        }
    }

    pub fn adopt(&mut self, post: &ExpectedPostState) {
        self.descriptor.state = post.descriptor_state;
        self.hoard.collateral_atoms = post.hoard_atoms;
        self.base.collateral = post.hoard_atoms;
        self.base.total_supply = post.total_supply;
        if let Some(position) = post.holder_position {
            self.source = position;
        }
        self.vault = post.vault_position;
        self.supply = post.supply;
        self.mint = post.mint;
        if let Some(token) = post.holder_token {
            self.token = token;
        }
        self.wrapper_replay = post.wrapper_replay;
        if let Some(replay) = post.source_replay {
            self.source_replay = replay;
        }
        self.vault_replay = post.vault_replay;
        self.request.wrapper_sequence = self.wrapper_replay.sequence;
        self.request.source_base_sequence = self.source_replay.sequence;
        self.request.vault_base_sequence = self.vault_replay.sequence;
        self.request.expected_mint_supply = self.mint.supply;
        self.request.expected_holder_amount = self.token.amount;
        self.accounts = self.full_accounts();
    }
}

fn access(
    role: AccountRole,
    key: Key,
    owner: Key,
    signer: bool,
    writable: bool,
    executable: bool,
) -> AccountAccess {
    AccountAccess {
        role,
        key,
        owner,
        signer,
        writable,
        executable,
    }
}

const _: () = assert!(MAX_CPI_STEPS == 3);
