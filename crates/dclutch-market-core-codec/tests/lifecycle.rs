//! Hostile generated Market Core ABI and sparse lifecycle coverage.

use dclutch_market_core_codec::{
    ACTION_ACTIVATE_CAPABILITY_TAG, ACTION_CLOSE_CAPABILITY_TAG, ACTION_VERIFY_READINESS_TAG,
    AccountCreation, Action, Admission, Binding, CapabilityChildObservation,
    ChildEffectObservation, ClaimsEffectObservation, CollateralObservation, CoreState, Error,
    FoundingAccounts, FoundingFrame, FoundingQuote, Holder, Identity, MarketIdentity, Phase,
    Product, REQUEST_BYTES, Readiness, Realm, ReleaseReceipt, ReleaseSet, Representation, Request,
    Role, STATE_BYTES, TerminalReceipt, VacantAccount, activate_capability_child, admit_terminal,
    begin_retiring, close_capability_child, found, open_market, redeem_terminal, retire,
    split_complete_set, verify_readiness,
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

fn realm() -> Realm {
    Realm {
        realm_id: id(20),
        collateral_mint: id(21),
        token_program: id(22),
        collateral_release: id(23),
    }
}

fn product(outcome_count: u32) -> Product {
    Product {
        product_id: id(30),
        result_domain: id(31),
        claim_basis: id(32),
        capacity_profile: id(33),
        compiler_release: id(34),
        outcome_count,
    }
}

fn identity() -> MarketIdentity {
    MarketIdentity {
        market_id: id(40),
        realm_id: id(20),
        product_id: id(30),
        result_domain: id(31),
        resolution_policy: id(41),
        capability_manifest: id(42),
        selected_release_set: id(10),
        generation: 7,
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

fn founding_frame(outcome_count: u32, market_lamports: u64) -> FoundingFrame {
    FoundingFrame {
        realm: realm(),
        product: product(outcome_count),
        identity: identity(),
        core_admission: admission(Role::Core),
        quote: FoundingQuote { market_rent: 100 },
        accounts: FoundingAccounts {
            payer_lamports: 5_000,
            rent_credit: id(46),
            market: vacant(id(40), market_lamports),
        },
    }
}

fn admin(action: Action, state: CoreState) -> Request {
    Request::administrative(action, state.identity.generation, state.identity.market_id)
}

fn child() -> ChildEffectObservation {
    ChildEffectObservation {
        exact_request_authenticated: true,
        exact_receipt_authenticated: true,
        post_resource_authenticated: true,
    }
}

fn claims(payout: u64, aggregate_empty: bool) -> ClaimsEffectObservation {
    ClaimsEffectObservation {
        child: child(),
        payout,
        aggregate_empty,
    }
}

fn capability(role: Role) -> CapabilityChildObservation {
    CapabilityChildObservation {
        target_role: role,
        admission: admission(role),
        manifest_entry_authenticated: true,
        funding_state_authenticated: true,
        effect: child(),
    }
}

fn collateral() -> CollateralObservation {
    let realm = realm();
    CollateralObservation {
        adapter_authenticated: true,
        realm_id: realm.realm_id,
        collateral_mint: realm.collateral_mint,
        token_program: realm.token_program,
        collateral_release: realm.collateral_release,
    }
}

fn ready_state(outcome_count: u32) -> CoreState {
    let mut result = found(
        Request::administrative(Action::Found, 7, id(40)),
        founding_frame(outcome_count, 7),
    )
    .expect("found succeeds");
    let request = admin(Action::VerifyReadiness, result.state);
    verify_readiness(
        request,
        &mut result.state,
        admission(Role::Core),
        true,
        child(),
    )
    .expect("readiness verifies");
    result.state
}

fn open_state(outcome_count: u32) -> CoreState {
    let mut state = ready_state(outcome_count);
    let request = admin(Action::OpenMarket, state);
    open_market(
        request,
        &mut state,
        admission(Role::Custody),
        realm(),
        true,
        true,
        collateral(),
        vacant(id(60), 20),
        70,
        true,
        child(),
    )
    .expect("market opens");
    state
}

fn terminal_receipt(selector: u32) -> TerminalReceipt {
    TerminalReceipt {
        receipt_id: id(80),
        market_id: id(40),
        resolution_policy: id(41),
        product_id: id(30),
        generation: 7,
        selector,
        authenticated: true,
    }
}

fn terminal_state(outcome_count: u32, selector: u32) -> CoreState {
    let mut state = open_state(outcome_count);
    let request = admin(Action::AdmitTerminal, state);
    admit_terminal(
        request,
        &mut state,
        admission(Role::Resolution),
        product(outcome_count),
        true,
        terminal_receipt(selector),
    )
    .expect("terminal receipt admits");
    state
}

#[test]
fn sparse_state_and_request_schema_are_fresh_and_hostile_decodable() {
    assert_eq!(STATE_BYTES, 320);
    assert_eq!(REQUEST_BYTES, 72);
    assert_eq!(ACTION_VERIFY_READINESS_TAG, 1);
    assert_eq!(ACTION_ACTIVATE_CAPABILITY_TAG, 8);
    assert_eq!(ACTION_CLOSE_CAPABILITY_TAG, 9);

    let mut state = open_state(17);
    state.outstanding_capabilities = 9;
    let encoded = state.encode().expect("state encodes");
    assert_eq!(CoreState::decode(&encoded), Ok(state));
    assert_eq!(
        u64::from_le_bytes(encoded[248..256].try_into().expect("count slice")),
        9
    );

    for action in [
        Action::VerifyReadiness,
        Action::ActivateCapability,
        Action::CloseCapability,
    ] {
        let request = admin(action, state);
        assert_eq!(
            Request::decode(&request.encode().expect("request encodes")),
            Ok(request)
        );
    }

    let mut old_magic = encoded;
    old_magic[7] = b'1';
    assert_eq!(CoreState::decode(&old_magic), Err(Error::InvalidMagic));
    assert_eq!(
        CoreState::decode(&encoded[..STATE_BYTES - 1]),
        Err(Error::InvalidLength)
    );

    let bytes = admin(Action::OpenMarket, state)
        .encode()
        .expect("request encodes");
    let mut reserved = bytes;
    reserved[13] = 1;
    assert_eq!(Request::decode(&reserved), Err(Error::NonzeroReserved));
    let mut unknown = bytes;
    unknown[10] = 10;
    assert_eq!(Request::decode(&unknown), Err(Error::InvalidTag));
}

#[test]
fn found_creates_only_the_dust_tolerant_core_market_account() {
    let result = found(
        Request::administrative(Action::Found, 7, id(40)),
        founding_frame(17, 7),
    )
    .expect("found succeeds");
    assert_eq!(
        result.plan.market,
        AccountCreation {
            before: 7,
            rent_minimum: 100,
            rent_top_up: 93,
            semantic_principal: 0,
            donation: 0,
            after: 100,
        }
    );
    assert_eq!(result.plan.payer_debit, 93);
    assert_eq!(result.plan.payer_after, 4_907);
    assert_eq!(result.state.rent_beneficiary, id(46));
    assert_eq!(result.state.outstanding_capabilities, 0);

    let donated = found(
        Request::administrative(Action::Found, 7, id(40)),
        founding_frame(17, 107),
    )
    .expect("above-rent vacancy is classified donation");
    assert_eq!(donated.plan.market.rent_top_up, 0);
    assert_eq!(donated.plan.market.donation, 7);
    assert_eq!(donated.plan.payer_debit, 0);

    let mut aliased = founding_frame(17, 0);
    aliased.accounts.rent_credit = aliased.accounts.market.address;
    assert_eq!(
        found(Request::administrative(Action::Found, 7, id(40)), aliased),
        Err(Error::InvalidCoordinates)
    );
}

#[test]
fn readiness_and_open_require_exact_external_child_evidence_before_commit() {
    let result = found(
        Request::administrative(Action::Found, 7, id(40)),
        founding_frame(3, 0),
    )
    .expect("found succeeds");
    let mut state = result.state;
    let before = state;
    let mut incomplete = child();
    incomplete.post_resource_authenticated = false;
    assert_eq!(
        verify_readiness(
            admin(Action::VerifyReadiness, state),
            &mut state,
            admission(Role::Core),
            true,
            incomplete
        ),
        Err(Error::InvalidChildEffect)
    );
    assert_eq!(state, before);
    verify_readiness(
        admin(Action::VerifyReadiness, state),
        &mut state,
        admission(Role::Core),
        true,
        child(),
    )
    .expect("readiness verifies");

    let before = state;
    let mut wrong = realm();
    wrong.collateral_mint = id(99);
    assert_eq!(
        open_market(
            admin(Action::OpenMarket, state),
            &mut state,
            admission(Role::Custody),
            wrong,
            true,
            true,
            collateral(),
            vacant(id(60), 20),
            70,
            true,
            child(),
        ),
        Err(Error::InvalidCoordinates)
    );
    assert_eq!(state, before);

    let creation = open_market(
        admin(Action::OpenMarket, state),
        &mut state,
        admission(Role::Custody),
        realm(),
        true,
        true,
        collateral(),
        vacant(id(60), 75),
        70,
        true,
        child(),
    )
    .expect("open accepts exact dust classification");
    assert_eq!(creation.rent_top_up, 0);
    assert_eq!(creation.donation, 5);
    assert_eq!(state.phase, Phase::Open);
    assert_eq!(state.readiness, Readiness::Consumed);
}

#[test]
fn split_is_runtime_width_and_never_mutates_sparse_core_state() {
    let state = open_state(17);
    let before = state;
    split_complete_set(
        Request::split(Holder::Source, Representation::Native, 10, 7, id(40)),
        &state,
        admission(Role::Claims),
        admission(Role::Custody),
        claims(0, false),
        child(),
    )
    .expect("runtime-width split effects authenticate");
    assert_eq!(state, before);

    let mut incomplete = child();
    incomplete.exact_receipt_authenticated = false;
    assert_eq!(
        split_complete_set(
            Request::split(Holder::Source, Representation::Native, 10, 7, id(40)),
            &state,
            admission(Role::Claims),
            admission(Role::Custody),
            claims(0, false),
            incomplete,
        ),
        Err(Error::InvalidChildEffect)
    );
}

#[test]
fn terminal_and_redemption_reauthenticate_product_without_funding_mirrors() {
    let mut state = open_state(17);
    let before = state;
    let mut wrong = terminal_receipt(16);
    wrong.product_id = id(99);
    assert_eq!(
        admit_terminal(
            admin(Action::AdmitTerminal, state),
            &mut state,
            admission(Role::Resolution),
            product(17),
            true,
            wrong
        ),
        Err(Error::InvalidTerminalReceipt)
    );
    assert_eq!(state, before);

    admit_terminal(
        admin(Action::AdmitTerminal, state),
        &mut state,
        admission(Role::Resolution),
        product(17),
        true,
        terminal_receipt(16),
    )
    .expect("exact Source-owned terminal receipt admits");
    assert_eq!(state.terminal_winner, 16);

    assert_eq!(
        redeem_terminal(
            Request::redeem(Holder::Source, Representation::Native, 16, 10, 7, id(40)),
            &state,
            admission(Role::Claims),
            admission(Role::Custody),
            product(17),
            true,
            claims(10, false),
            Some(child()),
        ),
        Ok(10)
    );
    assert_eq!(
        redeem_terminal(
            Request::redeem(Holder::Source, Representation::Native, 15, 10, 7, id(40)),
            &state,
            admission(Role::Claims),
            admission(Role::Custody),
            product(17),
            true,
            claims(0, false),
            None,
        ),
        Ok(0)
    );
    assert_eq!(
        redeem_terminal(
            Request::redeem(Holder::Source, Representation::Native, 16, 10, 7, id(40)),
            &state,
            admission(Role::Claims),
            admission(Role::Custody),
            product(17),
            false,
            claims(10, false),
            Some(child()),
        ),
        Err(Error::InvalidChildEffect)
    );
}

#[test]
fn generic_capability_count_has_one_manifest_funding_and_replay_boundary() {
    let mut state = open_state(3);
    let before = state;
    let mut forged = capability(Role::Trading);
    forged.manifest_entry_authenticated = false;
    assert_eq!(
        activate_capability_child(admin(Action::ActivateCapability, state), &mut state, forged),
        Err(Error::InvalidChildEffect)
    );
    assert_eq!(state, before);
    assert_eq!(
        activate_capability_child(
            admin(Action::ActivateCapability, state),
            &mut state,
            capability(Role::Core)
        ),
        Err(Error::InvalidChildEffect)
    );

    activate_capability_child(
        admin(Action::ActivateCapability, state),
        &mut state,
        capability(Role::Trading),
    )
    .expect("manifest-selected optional child activates");
    assert_eq!(state.outstanding_capabilities, 1);
    assert_eq!(
        CoreState::decode(&state.encode().expect("state encodes"))
            .expect("state decodes")
            .outstanding_capabilities,
        1
    );

    close_capability_child(
        admin(Action::CloseCapability, state),
        &mut state,
        capability(Role::Trading),
    )
    .expect("exact close receipt decrements");
    assert_eq!(state.outstanding_capabilities, 0);
    assert_eq!(
        close_capability_child(
            admin(Action::CloseCapability, state),
            &mut state,
            capability(Role::Trading)
        ),
        Err(Error::InvalidChildEffect)
    );

    state.outstanding_capabilities = u64::MAX;
    assert_eq!(
        activate_capability_child(
            admin(Action::ActivateCapability, state),
            &mut state,
            capability(Role::Trading)
        ),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(state.outstanding_capabilities, u64::MAX);
}

#[test]
fn retirement_requires_all_child_closures_and_returns_only_core_lamports() {
    let mut state = terminal_state(3, 1);
    begin_retiring(
        admin(Action::BeginRetiring, state),
        &mut state,
        admission(Role::Core),
    )
    .expect("begin retiring");
    state.outstanding_capabilities = 1;
    let before = state;
    assert_eq!(
        retire(
            admin(Action::Retire, state),
            &mut state,
            admission(Role::Core),
            admission(Role::Claims),
            admission(Role::Resolution),
            admission(Role::Custody),
            claims(0, true),
            child(),
            child(),
            123,
            true,
            true,
        ),
        Err(Error::InvalidChildEffect)
    );
    assert_eq!(state, before);

    state.outstanding_capabilities = 0;
    let before = state;
    assert_eq!(
        retire(
            admin(Action::Retire, state),
            &mut state,
            admission(Role::Core),
            admission(Role::Claims),
            admission(Role::Resolution),
            admission(Role::Custody),
            claims(0, true),
            child(),
            child(),
            123,
            false,
            true,
        ),
        Err(Error::InvalidAccount)
    );
    assert_eq!(state, before);

    assert_eq!(
        retire(
            admin(Action::Retire, state),
            &mut state,
            admission(Role::Core),
            admission(Role::Claims),
            admission(Role::Resolution),
            admission(Role::Custody),
            claims(0, true),
            child(),
            child(),
            123,
            true,
            true,
        ),
        Ok(123)
    );
    assert_eq!(state.phase, Phase::Retired);
}
