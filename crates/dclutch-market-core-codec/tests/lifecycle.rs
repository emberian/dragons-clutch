//! Hostile generated Market Core ABI and lifecycle coverage.

use dclutch_market_core_codec::{
    AccountCreation, Action, Admission, Binding, CollateralObservation, CoreCoordinates, CoreState,
    EconomicTail, EconomicVector, Error, FoundingAccounts, FoundingFrame, FoundingQuote,
    FundingState, Holder, Identity, MarketIdentity, Phase, Product, REQUEST_BYTES, Readiness,
    Realm, ReleaseReceipt, ReleaseSet, Representation, Request, Role, STATE_BYTES, TerminalReceipt,
    VacantAccount, activate_fund, admit_terminal, begin_retiring, found, open_market,
    redeem_terminal, retire, split_complete_set,
};

fn id(byte: u8) -> Identity {
    Identity::new([byte; 32]).expect("nonzero fixture identity")
}

fn binding(program: u8) -> Binding {
    Binding {
        program: id(program),
        artifact_release: id(program.saturating_add(20)),
        semantic_release: id(program.saturating_add(40)),
    }
}

fn release_set() -> ReleaseSet {
    ReleaseSet {
        release_set_id: id(10),
        bindings: [
            binding(11),
            binding(12),
            binding(13),
            binding(14),
            binding(15),
        ],
    }
}

fn selected_binding(selected: ReleaseSet, role: Role) -> Binding {
    let [core, claims, trading, resolution, custody] = selected.bindings;
    match role {
        Role::Core => core,
        Role::Claims => claims,
        Role::Trading => trading,
        Role::Resolution => resolution,
        Role::Custody => custody,
    }
}

fn admission(role: Role) -> Admission {
    let selected = release_set();
    Admission {
        market_release_set_id: selected.release_set_id,
        selected,
        receipt: ReleaseReceipt {
            registry_program: selected_binding(selected, Role::Core).program,
            release_set_id: selected.release_set_id,
            role,
            observed: selected_binding(selected, role),
            activation_cache_authenticated: true,
            current_deployment_reauthenticated: true,
        },
    }
}

fn vacant(address: Identity, lamports: u64) -> VacantAccount {
    VacantAccount {
        address,
        lamports,
        system_owned: true,
        data_empty: true,
        executable: false,
    }
}

fn founding_frame(outcome_count: u32) -> FoundingFrame {
    let realm = Realm {
        realm_id: id(20),
        collateral_mint: id(21),
        token_program: id(22),
        collateral_release: id(23),
    };
    let product = Product {
        product_id: id(30),
        result_domain: id(31),
        claim_basis: id(32),
        capacity_profile: id(33),
        compiler_release: id(34),
        outcome_count,
        scalar_limit: 1_000,
    };
    let identity = MarketIdentity {
        market_id: id(40),
        realm_id: realm.realm_id,
        product_id: product.product_id,
        result_domain: product.result_domain,
        resolution_policy: id(41),
        selected_release_set: release_set().release_set_id,
        generation: 7,
    };
    let coordinates = CoreCoordinates {
        derivation_authenticated: true,
        market: identity.market_id,
        hoard: id(42),
        fund: id(43),
        readiness: id(44),
        custody: id(45),
        rent_credit: id(46),
    };
    FoundingFrame {
        realm,
        product,
        identity,
        core_admission: admission(Role::Core),
        coordinates,
        quote: FoundingQuote {
            market_rent: 100,
            hoard_rent: 50,
            fund_rent: 80,
            readiness_rent: 30,
            custody_rent: 70,
            source_funding_allocation: id(50),
            source_work_capital: 500,
        },
        accounts: FoundingAccounts {
            payer_lamports: 5_000,
            rent_credit: coordinates.rent_credit,
            rent_credit_lamports: 100,
            market: vacant(coordinates.market, 7),
            hoard: vacant(coordinates.hoard, 55),
            fund: vacant(coordinates.fund, 10),
            readiness: vacant(coordinates.readiness, 35),
        },
    }
}

fn admin(action: Action, state: CoreState) -> Request {
    Request::administrative(action, state.identity.generation, state.identity.market_id)
}

fn collateral(state: CoreState) -> CollateralObservation {
    CollateralObservation {
        adapter_authenticated: true,
        realm_id: state.realm.realm_id,
        collateral_mint: state.realm.collateral_mint,
        token_program: state.realm.token_program,
        collateral_release: state.realm.collateral_release,
    }
}

fn ready_open_state(outcome_count: u32) -> CoreState {
    let frame = founding_frame(outcome_count);
    let mut result =
        found(Request::administrative(Action::Found, 7, id(40)), frame).expect("found succeeds");
    activate_fund(
        admin(Action::ActivateFund, result.state),
        &mut result.state,
        admission(Role::Resolution),
        result.source_funding,
    )
    .expect("fund becomes ready");
    let state_before = result.state;
    open_market(
        admin(Action::OpenMarket, state_before),
        &mut result.state,
        admission(Role::Custody),
        collateral(state_before),
        vacant(state_before.coordinates.custody, 20),
    )
    .expect("market opens");
    result.state
}

struct Vectors {
    bytes: Vec<u8>,
    outcome_count: u32,
}

impl Vectors {
    fn zero(width: usize) -> Self {
        let outcome_count = u32::try_from(width).expect("fixture width fits u32");
        Self {
            bytes: vec![0; EconomicTail::byte_len(outcome_count).expect("fixture tail width")],
            outcome_count,
        }
    }

    fn view(&mut self) -> EconomicTail<'_> {
        EconomicTail::new(&mut self.bytes, self.outcome_count).expect("fixture tail validates")
    }

    fn all_equal(&mut self, vector: EconomicVector, expected: u64) -> bool {
        let outcome_count = self.outcome_count;
        let view = self.view();
        (0..outcome_count).all(|outcome| {
            view.value(vector, outcome)
                .is_ok_and(|value| value == expected)
        })
    }

    fn value_count(&self) -> usize {
        usize::try_from(self.outcome_count).expect("fixture width fits usize")
    }

    fn snapshot(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    fn is_zero(&mut self) -> bool {
        for vector in [
            EconomicVector::Supply,
            EconomicVector::NativeSupply,
            EconomicVector::MaterializedSupply,
            EconomicVector::SourceNative,
            EconomicVector::SourceMaterialized,
            EconomicVector::DestinationNative,
            EconomicVector::DestinationMaterialized,
        ] {
            if !self.all_equal(vector, 0) {
                return false;
            }
        }
        true
    }
}

#[test]
fn funded_lifecycle_preserves_every_compartment_and_round_trips() {
    let frame = founding_frame(4);
    let mut result =
        found(Request::administrative(Action::Found, 7, id(40)), frame).expect("found succeeds");
    assert_eq!(result.plan.payer_debit, 733);
    assert_eq!(result.plan.payer_after, 4_267);
    assert_eq!(result.plan.market.rent_top_up, 93);
    assert_eq!(result.plan.hoard.donation, 5);
    assert_eq!(result.plan.fund.semantic_principal, 570);
    assert_eq!(result.plan.readiness.donation, 5);
    assert_eq!(result.state.hoard_principal, 0);
    assert_eq!(result.state.capital.deferred_custody_rent, 70);
    assert_eq!(result.source_funding.remaining_capital, 500);

    activate_fund(
        admin(Action::ActivateFund, result.state),
        &mut result.state,
        admission(Role::Resolution),
        result.source_funding,
    )
    .expect("fund becomes ready");
    assert_eq!(result.state.readiness, Readiness::Ready);

    let state_before_open = result.state;
    let top_up = open_market(
        admin(Action::OpenMarket, state_before_open),
        &mut result.state,
        admission(Role::Custody),
        collateral(state_before_open),
        vacant(state_before_open.coordinates.custody, 20),
    )
    .expect("market opens");
    assert_eq!(top_up, 50);
    assert_eq!(result.state.phase, Phase::Open);
    assert_eq!(result.state.capital.custody_rent, 70);
    assert_eq!(result.state.capital.custody_donation, 0);
    assert_eq!(result.state.capital.rent_credit, 155);

    let mut vectors = Vectors::zero(4);
    split_complete_set(
        Request::split(
            Holder::Source,
            Representation::Native,
            10,
            result.state.identity.generation,
            result.state.identity.market_id,
        ),
        &mut result.state,
        &mut vectors.view(),
        admission(Role::Claims),
        admission(Role::Custody),
    )
    .expect("complete set splits");
    assert!(vectors.all_equal(EconomicVector::Supply, 10));
    assert!(vectors.all_equal(EconomicVector::NativeSupply, 10));
    assert!(vectors.all_equal(EconomicVector::SourceNative, 10));
    assert_eq!(result.state.hoard_principal, 10);

    let receipt = TerminalReceipt {
        receipt_id: id(60),
        market_id: result.state.identity.market_id,
        resolution_policy: result.state.identity.resolution_policy,
        product_id: result.state.product.product_id,
        generation: result.state.identity.generation,
        selector: 1,
        funding_allocation: result.state.funding.allocation_id,
        funding_remaining: 400,
        authenticated: true,
    };
    admit_terminal(
        admin(Action::AdmitTerminal, result.state),
        &mut result.state,
        admission(Role::Resolution),
        receipt,
        &vectors.view(),
    )
    .expect("terminal receipt admits");
    assert_eq!(result.state.phase, Phase::Terminal);
    assert_eq!(result.state.terminal_winner, 1);

    begin_retiring(
        admin(Action::BeginRetiring, result.state),
        &mut result.state,
        admission(Role::Core),
    )
    .expect("retirement begins");
    for outcome in 0_u32..4 {
        let request = Request::redeem(
            Holder::Source,
            Representation::Native,
            outcome,
            10,
            result.state.identity.generation,
            result.state.identity.market_id,
        );
        let payout = redeem_terminal(
            request,
            &mut result.state,
            &mut vectors.view(),
            admission(Role::Claims),
            admission(Role::Custody),
        )
        .expect("claim redeems");
        assert_eq!(payout, u64::from(outcome == 1) * 10);
    }
    assert_eq!(result.state.hoard_principal, 0);
    assert!(vectors.is_zero());

    let funding = FundingState {
        allocation_id: result.state.funding.allocation_id,
        initial_capital: 500,
        remaining_capital: 400,
        paid_capital: 100,
        call_count: 1,
    };
    let refund = retire(
        admin(Action::Retire, result.state),
        &mut result.state,
        &vectors.view(),
        admission(Role::Core),
        admission(Role::Custody),
        funding,
    )
    .expect("empty market retires");
    assert_eq!(refund, 705);
    assert_eq!(result.state.phase, Phase::Retired);
    assert_eq!(result.state.capital.rent_credit, 860);
    let encoded = result.state.encode().expect("state encodes");
    assert_eq!(encoded.len(), STATE_BYTES);
    assert_eq!(CoreState::decode(&encoded), Ok(result.state));
}

#[test]
fn runtime_product_width_has_no_width_specialized_branch() {
    let mut state = ready_open_state(17);
    let mut vectors = Vectors::zero(17);
    split_complete_set(
        Request::split(
            Holder::Destination,
            Representation::Materialized,
            3,
            state.identity.generation,
            state.identity.market_id,
        ),
        &mut state,
        &mut vectors.view(),
        admission(Role::Claims),
        admission(Role::Custody),
    )
    .expect("runtime width executes");
    assert_eq!(vectors.value_count(), 17);
    assert!(vectors.all_equal(EconomicVector::Supply, 3));
    assert!(vectors.all_equal(EconomicVector::MaterializedSupply, 3));
    assert!(vectors.all_equal(EconomicVector::DestinationMaterialized, 3));
}

#[test]
fn hostile_request_and_state_bytes_are_refused() {
    let state = ready_open_state(4);
    let request = Request::split(
        Holder::Source,
        Representation::Native,
        10,
        state.identity.generation,
        state.identity.market_id,
    );
    let encoded = request.encode().expect("request encodes");
    assert_eq!(encoded.len(), REQUEST_BYTES);
    assert_eq!(Request::decode(&encoded), Ok(request));

    let mut bad_magic = encoded;
    *bad_magic.first_mut().expect("request has magic") ^= 1;
    assert_eq!(Request::decode(&bad_magic), Err(Error::InvalidMagic));

    let mut bad_reserved = encoded;
    *bad_reserved.get_mut(13).expect("reserved byte exists") = 1;
    assert_eq!(Request::decode(&bad_reserved), Err(Error::NonzeroReserved));

    let mut bad_tag = encoded;
    *bad_tag.get_mut(10).expect("action tag exists") = u8::MAX;
    assert_eq!(Request::decode(&bad_tag), Err(Error::InvalidTag));
    assert_eq!(
        Request::decode(&encoded[..REQUEST_BYTES - 1]),
        Err(Error::InvalidLength)
    );

    let state_bytes = state.encode().expect("state encodes");
    let mut state_reserved = state_bytes;
    *state_reserved
        .get_mut(308)
        .expect("product reserved byte exists") = 1;
    assert_eq!(
        CoreState::decode(&state_reserved),
        Err(Error::NonzeroReserved)
    );

    let mut substituted_realm = state_bytes;
    substituted_realm
        .get_mut(352..384)
        .expect("identity Realm field exists")
        .copy_from_slice(&id(99).to_bytes());
    assert_eq!(
        CoreState::decode(&substituted_realm),
        Err(Error::InvalidPhase)
    );
}

#[test]
fn found_refuses_release_identity_coordinate_and_account_substitution() {
    let request = Request::administrative(Action::Found, 7, id(40));

    let mut wrong_realm = founding_frame(4);
    wrong_realm.identity.realm_id = id(99);
    assert_eq!(found(request, wrong_realm), Err(Error::InvalidFunding));

    let mut wrong_rent_credit = founding_frame(4);
    wrong_rent_credit.accounts.rent_credit = id(99);
    assert_eq!(
        found(request, wrong_rent_credit),
        Err(Error::InvalidCoordinates)
    );

    let mut aliased = founding_frame(4);
    aliased.coordinates.hoard = aliased.coordinates.market;
    assert_eq!(found(request, aliased), Err(Error::InvalidFunding));

    let mut substituted_release = founding_frame(4);
    substituted_release.core_admission.receipt.observed = binding(99);
    assert_eq!(
        found(request, substituted_release),
        Err(Error::InvalidFunding)
    );

    let mut release_alias = founding_frame(4);
    let mut bindings = release_alias.core_admission.selected.bindings;
    let claims = bindings.get_mut(1).expect("claims binding exists");
    claims.program = selected_binding(release_alias.core_admission.selected, Role::Core).program;
    release_alias.core_admission.selected.bindings = bindings;
    assert_eq!(found(request, release_alias), Err(Error::InvalidFunding));
}

#[test]
fn failed_transitions_are_atomic() {
    let mut state = ready_open_state(4);
    let state_before = state;
    let mut vectors = Vectors::zero(4);
    split_complete_set(
        Request::split(
            Holder::Source,
            Representation::Native,
            990,
            state.identity.generation,
            state.identity.market_id,
        ),
        &mut state,
        &mut vectors.view(),
        admission(Role::Claims),
        admission(Role::Custody),
    )
    .expect("large valid split seeds overflow boundary");
    let vectors_before = vectors.snapshot();
    let error = split_complete_set(
        Request::split(
            Holder::Source,
            Representation::Native,
            10,
            state.identity.generation,
            state.identity.market_id,
        ),
        &mut state,
        &mut vectors.view(),
        admission(Role::Claims),
        admission(Role::Custody),
    );
    assert_eq!(error, Err(Error::ArithmeticOverflow));
    assert_eq!(state.hoard_principal, 990);
    assert_eq!(state.phase, state_before.phase);
    assert_eq!(vectors.snapshot(), vectors_before);

    state = ready_open_state(4);
    vectors = Vectors::zero(4);
    let state_before_receipt = state;
    let bad_receipt = TerminalReceipt {
        receipt_id: id(60),
        market_id: state.identity.market_id,
        resolution_policy: id(99),
        product_id: state.product.product_id,
        generation: state.identity.generation,
        selector: 1,
        funding_allocation: state.funding.allocation_id,
        funding_remaining: 400,
        authenticated: true,
    };
    assert_eq!(
        admit_terminal(
            admin(Action::AdmitTerminal, state),
            &mut state,
            admission(Role::Resolution),
            bad_receipt,
            &vectors.view(),
        ),
        Err(Error::InvalidTerminalReceipt)
    );
    assert_eq!(state, state_before_receipt);

    let mut founding = found(
        Request::administrative(Action::Found, 7, id(40)),
        founding_frame(4),
    )
    .expect("found succeeds")
    .state;
    activate_fund(
        admin(Action::ActivateFund, founding),
        &mut founding,
        admission(Role::Resolution),
        FundingState {
            allocation_id: id(50),
            initial_capital: 500,
            remaining_capital: 500,
            paid_capital: 0,
            call_count: 0,
        },
    )
    .expect("fund becomes ready");
    let founding_before = founding;
    let mut wrong_collateral = collateral(founding);
    wrong_collateral.collateral_mint = id(99);
    assert_eq!(
        open_market(
            admin(Action::OpenMarket, founding),
            &mut founding,
            admission(Role::Custody),
            wrong_collateral,
            vacant(founding_before.coordinates.custody, 0),
        ),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(founding, founding_before);
}

#[test]
fn retirement_requires_terminal_zero_liabilities_and_exact_funding() {
    let mut state = ready_open_state(3);
    let mut vectors = Vectors::zero(3);
    split_complete_set(
        Request::split(
            Holder::Source,
            Representation::Native,
            5,
            state.identity.generation,
            state.identity.market_id,
        ),
        &mut state,
        &mut vectors.view(),
        admission(Role::Claims),
        admission(Role::Custody),
    )
    .expect("split succeeds");
    let receipt = TerminalReceipt {
        receipt_id: id(60),
        market_id: state.identity.market_id,
        resolution_policy: state.identity.resolution_policy,
        product_id: state.product.product_id,
        generation: state.identity.generation,
        selector: 0,
        funding_allocation: state.funding.allocation_id,
        funding_remaining: 400,
        authenticated: true,
    };
    admit_terminal(
        admin(Action::AdmitTerminal, state),
        &mut state,
        admission(Role::Resolution),
        receipt,
        &vectors.view(),
    )
    .expect("terminal admits");
    assert_eq!(
        retire(
            admin(Action::Retire, state),
            &mut state,
            &vectors.view(),
            admission(Role::Core),
            admission(Role::Custody),
            FundingState {
                allocation_id: id(50),
                initial_capital: 500,
                remaining_capital: 400,
                paid_capital: 100,
                call_count: 1,
            },
        ),
        Err(Error::InvalidPhase)
    );
    begin_retiring(
        admin(Action::BeginRetiring, state),
        &mut state,
        admission(Role::Core),
    )
    .expect("retirement begins");
    let before = state;
    assert_eq!(
        retire(
            admin(Action::Retire, state),
            &mut state,
            &vectors.view(),
            admission(Role::Core),
            admission(Role::Custody),
            FundingState {
                allocation_id: id(50),
                initial_capital: 500,
                remaining_capital: 400,
                paid_capital: 100,
                call_count: 1,
            },
        ),
        Err(Error::InvalidEconomicState)
    );
    assert_eq!(state, before);
}

#[test]
fn account_creation_shape_is_exact_and_dust_tolerant() {
    let frame = founding_frame(4);
    let result =
        found(Request::administrative(Action::Found, 7, id(40)), frame).expect("found succeeds");
    assert_eq!(
        result.plan.hoard,
        AccountCreation {
            before: 55,
            rent_minimum: 50,
            rent_top_up: 0,
            semantic_principal: 0,
            donation: 5,
            after: 55,
        }
    );
}
