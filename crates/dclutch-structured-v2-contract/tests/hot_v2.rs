//! Acceptance and refusal evidence for the opaque Structured V2 Hot candidate.

mod support;

use dclutch_fractional_claim_kernel::FractionalExposureTermsV2;
use dclutch_structured_v2_contract::{
    StructuredActionV2, StructuredHotAccountRefV2, StructuredHotCandidateInputV2,
    StructuredHotCandidateV2, StructuredHotErrorV2, StructuredHotRentCloseV2,
    StructuredHotTokenEffectV2, StructuredHotTokenKindV2, StructuredHotTokenPostV2,
    StructuredRequestInputV2, StructuredRequestV2,
};
use dclutch_structured_v2_kernel::{STRUCTURED_NO_COORDINATE_V2, StructuredTermsV2};
use support::{
    CUSTODY_SHARD_BASE, DENOMINATOR_FIXTURE, HOLDER_SHARD_BASE, MARKET, OWNER, PRODUCT_RECORD,
    RECEIPT_DESTINATION, RECEIPT_MINT, RECEIPT_SOURCE, RECEIPT_TOKEN_BEHAVIOR, RELEASE_SET,
    RENT_CREDIT, RENT_PROGRAM, RESULT_DOMAIN, ROOT, SHARD_EXPOSURE, TERMINAL_DIGEST, TOKEN_PROGRAM,
    digest, identity, root_bytes, shard_mints, shard_terms, shard_terms_bytes, structured_terms,
    structured_terms_bytes,
};

const COEFFICIENTS: [u64; 2] = [1, 3];
const QUANTITY: u64 = 4;

/// Profile coordinates used by every fixture below.
const TOKEN_PROGRAM_COORDINATE: u16 = 0;
const ROOT_COORDINATE: u16 = 1;
const OWNER_COORDINATE: u16 = 2;
const RECEIPT_MINT_COORDINATE: u16 = 3;
const RECEIPT_ACCOUNT_COORDINATE: u16 = 4;
const SHARD_MINT_COORDINATE_BASE: u16 = 5;
const HOLDER_COORDINATE_BASE: u16 = 10;
const CUSTODY_COORDINATE_BASE: u16 = 20;
const RENT_PROGRAM_COORDINATE: u16 = 30;
const RENT_CREDIT_COORDINATE: u16 = 31;

fn account(coordinate: u16, tag: u8) -> StructuredHotAccountRefV2 {
    StructuredHotAccountRefV2::new(coordinate, identity(tag)).expect("account ref")
}

fn account_key(coordinate: u16, key: [u8; 32]) -> StructuredHotAccountRefV2 {
    StructuredHotAccountRefV2::new(coordinate, key).expect("account ref")
}

fn request(action: StructuredActionV2, terms: StructuredTermsV2<'_>) -> StructuredRequestV2 {
    let carries = matches!(
        action,
        StructuredActionV2::Issue | StructuredActionV2::Unwrap | StructuredActionV2::TerminalRedeem
    );
    StructuredRequestV2::new(
        action,
        StructuredRequestInputV2 {
            release_set: identity(RELEASE_SET),
            market: identity(MARKET),
            product_record: identity(PRODUCT_RECORD),
            result_domain: identity(RESULT_DOMAIN),
            terms: terms.terms_id(),
            token_behavior: identity(RECEIPT_TOKEN_BEHAVIOR),
            shard_terms: terms.shard_terms(),
            shard_exposure: identity(SHARD_EXPOSURE),
            owner: if carries { identity(OWNER) } else { [0; 32] },
            receipt_source: if action == StructuredActionV2::Issue || !carries {
                [0; 32]
            } else {
                identity(RECEIPT_SOURCE)
            },
            receipt_destination: if action == StructuredActionV2::Issue {
                identity(RECEIPT_DESTINATION)
            } else {
                [0; 32]
            },
            terminal_digest: if matches!(
                action,
                StructuredActionV2::TerminalRedeem | StructuredActionV2::ZeroSupplyRetire
            ) {
                identity(TERMINAL_DIGEST)
            } else {
                [0; 32]
            },
            expected_revision: 7,
            quantity: if carries { QUANTITY } else { 0 },
        },
    )
    .expect("request")
    .bind_terms(terms)
    .expect("bound request")
}

fn shard_effect(
    coordinate: u32,
    kind: StructuredHotTokenKindV2,
    amount: u64,
    authority: StructuredHotAccountRefV2,
) -> StructuredHotTokenEffectV2 {
    let index = u16::try_from(coordinate).expect("coordinate");
    let mint = shard_mints(2)
        .get(usize::from(index))
        .copied()
        .expect("shard mint");
    let tag = u8::try_from(index).expect("coordinate tag");
    let holder = account_key(
        HOLDER_COORDINATE_BASE + index,
        identity(HOLDER_SHARD_BASE + tag),
    );
    let custody = account_key(
        CUSTODY_COORDINATE_BASE + index,
        identity(CUSTODY_SHARD_BASE + tag),
    );
    let (source, destination) = if kind == StructuredHotTokenKindV2::LockShards {
        (holder, custody)
    } else {
        (custody, holder)
    };
    StructuredHotTokenEffectV2 {
        kind,
        representation_coordinate: coordinate,
        token_program: account(TOKEN_PROGRAM_COORDINATE, TOKEN_PROGRAM),
        mint: account_key(SHARD_MINT_COORDINATE_BASE + index, mint),
        source: Some(source),
        destination: Some(destination),
        authority,
        amount,
        pre_supply: 0,
        post_supply: 0,
        pre_source: 1_000,
        post_source: 1_000 - amount,
        pre_destination: 500,
        post_destination: 500 + amount,
    }
}

fn receipt_effect(
    kind: StructuredHotTokenKindV2,
    root: StructuredHotAccountRefV2,
) -> StructuredHotTokenEffectV2 {
    let minting = kind == StructuredHotTokenKindV2::MintReceipts;
    StructuredHotTokenEffectV2 {
        kind,
        representation_coordinate: STRUCTURED_NO_COORDINATE_V2,
        token_program: account(TOKEN_PROGRAM_COORDINATE, TOKEN_PROGRAM),
        mint: account(RECEIPT_MINT_COORDINATE, RECEIPT_MINT),
        source: if minting {
            None
        } else {
            Some(account(RECEIPT_ACCOUNT_COORDINATE, RECEIPT_SOURCE))
        },
        destination: if minting {
            Some(account(RECEIPT_ACCOUNT_COORDINATE, RECEIPT_DESTINATION))
        } else {
            None
        },
        authority: root,
        amount: QUANTITY,
        pre_supply: 10,
        post_supply: if minting { 14 } else { 6 },
        pre_source: if minting { 0 } else { 10 },
        post_source: if minting { 0 } else { 6 },
        pre_destination: if minting { 10 } else { 0 },
        post_destination: if minting { 14 } else { 0 },
    }
}

fn issue_effects(root: StructuredHotAccountRefV2) -> Vec<StructuredHotTokenEffectV2> {
    vec![
        receipt_effect(StructuredHotTokenKindV2::MintReceipts, root),
        shard_effect(
            0,
            StructuredHotTokenKindV2::LockShards,
            QUANTITY,
            account(OWNER_COORDINATE, OWNER),
        ),
        shard_effect(
            1,
            StructuredHotTokenKindV2::LockShards,
            QUANTITY * 3,
            account(OWNER_COORDINATE, OWNER),
        ),
    ]
}

fn release_effects(root: StructuredHotAccountRefV2) -> Vec<StructuredHotTokenEffectV2> {
    vec![
        receipt_effect(StructuredHotTokenKindV2::BurnReceipts, root),
        shard_effect(0, StructuredHotTokenKindV2::ReleaseShards, QUANTITY, root),
        shard_effect(
            1,
            StructuredHotTokenKindV2::ReleaseShards,
            QUANTITY * 3,
            root,
        ),
    ]
}

fn closure_effect(
    coordinate: u32,
    root: StructuredHotAccountRefV2,
    rent_credit: StructuredHotAccountRefV2,
) -> StructuredHotTokenEffectV2 {
    let index = u16::try_from(coordinate).expect("coordinate");
    let mint = shard_mints(2)
        .get(usize::from(index))
        .copied()
        .expect("shard mint");
    StructuredHotTokenEffectV2 {
        kind: StructuredHotTokenKindV2::CloseCustody,
        representation_coordinate: coordinate,
        token_program: account(TOKEN_PROGRAM_COORDINATE, TOKEN_PROGRAM),
        mint: account_key(SHARD_MINT_COORDINATE_BASE + index, mint),
        source: Some(account_key(
            CUSTODY_COORDINATE_BASE + index,
            identity(CUSTODY_SHARD_BASE + u8::try_from(index).expect("coordinate tag")),
        )),
        destination: Some(rent_credit),
        authority: root,
        amount: 0,
        pre_supply: 7,
        post_supply: 7,
        pre_source: 0,
        post_source: 0,
        pre_destination: 0,
        post_destination: 0,
    }
}

fn retirement_effects(
    root: StructuredHotAccountRefV2,
    rent_credit: StructuredHotAccountRefV2,
) -> Vec<StructuredHotTokenEffectV2> {
    vec![
        closure_effect(0, root, rent_credit),
        closure_effect(1, root, rent_credit),
        StructuredHotTokenEffectV2 {
            kind: StructuredHotTokenKindV2::CloseReceiptMint,
            representation_coordinate: STRUCTURED_NO_COORDINATE_V2,
            token_program: account(TOKEN_PROGRAM_COORDINATE, TOKEN_PROGRAM),
            mint: account(RECEIPT_MINT_COORDINATE, RECEIPT_MINT),
            source: None,
            destination: Some(rent_credit),
            authority: root,
            amount: 0,
            pre_supply: 0,
            post_supply: 0,
            pre_source: 0,
            post_source: 0,
            pre_destination: 0,
            post_destination: 0,
        },
    ]
}

fn rent_close(rent_credit: StructuredHotAccountRefV2) -> StructuredHotRentCloseV2 {
    StructuredHotRentCloseV2 {
        rent_program: account(RENT_PROGRAM_COORDINATE, RENT_PROGRAM),
        rent_credit,
        route_base: 40,
        post_resource_digest: identity(0x99),
    }
}

struct Fixture {
    shard_bytes: Vec<u8>,
    terms_bytes: Vec<u8>,
    root_bytes: Vec<u8>,
}

impl Fixture {
    fn new() -> Self {
        let shard_bytes = shard_terms_bytes(2, DENOMINATOR_FIXTURE);
        let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR_FIXTURE);
        let root_bytes = root_bytes(digest(&terms_bytes), 7);
        Self {
            shard_bytes,
            terms_bytes,
            root_bytes,
        }
    }

    fn shard(&self) -> FractionalExposureTermsV2<'_> {
        shard_terms(&self.shard_bytes)
    }

    fn terms(&self) -> StructuredTermsV2<'_> {
        structured_terms(&self.terms_bytes, self.shard())
    }
}

#[test]
fn issue_candidate_is_accepted_and_advances_the_root() {
    let fixture = Fixture::new();
    let terms = fixture.terms();
    let root = account(ROOT_COORDINATE, ROOT);
    let effects = issue_effects(root);
    let candidate = StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
        request: request(StructuredActionV2::Issue, terms),
        terms,
        shard_terms: fixture.shard(),
        root_bytes: &fixture.root_bytes,
        root,
        token_effects: &effects,
        rent_close: None,
    })
    .expect("issue candidate");
    assert_eq!(candidate.action(), StructuredActionV2::Issue);
    assert_eq!(candidate.pre_revision(), 7);
    assert_eq!(candidate.post_revision(), Some(8));
    let expected = candidate.root_candidate_bytes().expect("root candidate");
    assert_eq!(candidate.validate_root_poststate(Some(&expected)), Ok(()));
    assert_eq!(
        candidate.validate_root_poststate(Some(&fixture.root_bytes)),
        Err(StructuredHotErrorV2::RootMismatch)
    );
    assert_eq!(
        candidate.validate_root_poststate(None),
        Err(StructuredHotErrorV2::RootMismatch)
    );
}

#[test]
fn release_candidates_are_accepted_in_their_own_actions() {
    let fixture = Fixture::new();
    let terms = fixture.terms();
    let root = account(ROOT_COORDINATE, ROOT);
    let effects = release_effects(root);
    for action in [
        StructuredActionV2::Unwrap,
        StructuredActionV2::TerminalRedeem,
    ] {
        let candidate = StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
            request: request(action, terms),
            terms,
            shard_terms: fixture.shard(),
            root_bytes: &fixture.root_bytes,
            root,
            token_effects: &effects,
            rent_close: None,
        })
        .expect("release candidate");
        assert_eq!(candidate.action(), action);
        assert_eq!(candidate.token_effects().len(), 3);
    }
}

#[test]
fn retirement_candidate_closes_custody_then_the_receipt_mint() {
    let fixture = Fixture::new();
    let terms = fixture.terms();
    let root = account(ROOT_COORDINATE, ROOT);
    let rent_credit = account(RENT_CREDIT_COORDINATE, RENT_CREDIT);
    let effects = retirement_effects(root, rent_credit);
    let candidate = StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
        request: request(StructuredActionV2::ZeroSupplyRetire, terms),
        terms,
        shard_terms: fixture.shard(),
        root_bytes: &fixture.root_bytes,
        root,
        token_effects: &effects,
        rent_close: Some(rent_close(rent_credit)),
    })
    .expect("retire candidate");
    assert_eq!(candidate.post_revision(), None);
    assert_eq!(candidate.root_candidate_bytes(), None);
    assert_eq!(candidate.validate_root_poststate(None), Ok(()));
    assert_eq!(
        candidate.validate_root_poststate(Some(&fixture.root_bytes)),
        Err(StructuredHotErrorV2::RootMismatch)
    );
}

#[test]
fn token_poststate_must_match_the_candidate_exactly() {
    let fixture = Fixture::new();
    let terms = fixture.terms();
    let root = account(ROOT_COORDINATE, ROOT);
    let effects = issue_effects(root);
    let candidate = StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
        request: request(StructuredActionV2::Issue, terms),
        terms,
        shard_terms: fixture.shard(),
        root_bytes: &fixture.root_bytes,
        root,
        token_effects: &effects,
        rent_close: None,
    })
    .expect("issue candidate");
    let observed: Vec<StructuredHotTokenPostV2> = effects
        .iter()
        .map(|effect| StructuredHotTokenPostV2 {
            representation_coordinate: effect.representation_coordinate,
            mint: effect.mint.key(),
            supply: effect.post_supply,
            source_amount: effect.post_source,
            destination_amount: effect.post_destination,
        })
        .collect();
    assert_eq!(candidate.validate_token_poststate(&observed), Ok(()));

    let mut short = observed.clone();
    short.pop();
    assert_eq!(
        candidate.validate_token_poststate(&short),
        Err(StructuredHotErrorV2::TokenMismatch)
    );

    let mut drifted = observed;
    if let Some(first) = drifted.first_mut() {
        first.supply += 1;
    }
    assert_eq!(
        candidate.validate_token_poststate(&drifted),
        Err(StructuredHotErrorV2::TokenMismatch)
    );
}

#[test]
fn hostile_candidates_refuse() {
    let fixture = Fixture::new();
    let terms = fixture.terms();
    let root = account(ROOT_COORDINATE, ROOT);
    let rent_credit = account(RENT_CREDIT_COORDINATE, RENT_CREDIT);

    let prepare = |effects: &[StructuredHotTokenEffectV2],
                   action: StructuredActionV2,
                   rent: Option<StructuredHotRentCloseV2>,
                   root_bytes: &[u8]| {
        StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
            request: request(action, terms),
            terms,
            shard_terms: fixture.shard(),
            root_bytes,
            root,
            token_effects: effects,
            rent_close: rent,
        })
        .err()
    };

    // A missing shard effect refuses on count.
    let mut missing = issue_effects(root);
    missing.pop();
    assert_eq!(
        prepare(
            &missing,
            StructuredActionV2::Issue,
            None,
            &fixture.root_bytes
        ),
        Some(StructuredHotErrorV2::TokenMismatch)
    );

    // A basket amount that ignores the coefficient refuses.
    let mut wrong_amount = issue_effects(root);
    if let Some(effect) = wrong_amount.get_mut(2) {
        effect.amount = QUANTITY;
        effect.post_source = 1_000 - QUANTITY;
        effect.post_destination = 500 + QUANTITY;
    }
    assert_eq!(
        prepare(
            &wrong_amount,
            StructuredActionV2::Issue,
            None,
            &fixture.root_bytes
        ),
        Some(StructuredHotErrorV2::TokenMismatch)
    );

    // A lock signed by the root instead of the actor refuses.
    let mut wrong_authority = issue_effects(root);
    if let Some(effect) = wrong_authority.get_mut(1) {
        effect.authority = root;
    }
    assert_eq!(
        prepare(
            &wrong_authority,
            StructuredActionV2::Issue,
            None,
            &fixture.root_bytes
        ),
        Some(StructuredHotErrorV2::TokenMismatch)
    );

    // A substituted shard Mint refuses.
    let mut wrong_mint = issue_effects(root);
    if let Some(effect) = wrong_mint.get_mut(1) {
        effect.mint = account_key(SHARD_MINT_COORDINATE_BASE, identity(0x7f));
    }
    assert_eq!(
        prepare(
            &wrong_mint,
            StructuredActionV2::Issue,
            None,
            &fixture.root_bytes
        ),
        Some(StructuredHotErrorV2::TokenMismatch)
    );

    // A descending Mint coordinate refuses; the frame must be canonically ordered.
    let mut unordered = issue_effects(root);
    if let Some(effect) = unordered.get_mut(2) {
        effect.mint = account_key(
            SHARD_MINT_COORDINATE_BASE,
            shard_mints(2).get(1).copied().expect("mint"),
        );
    }
    assert_eq!(
        prepare(
            &unordered,
            StructuredActionV2::Issue,
            None,
            &fixture.root_bytes
        ),
        Some(StructuredHotErrorV2::AccountMismatch)
    );

    // A stale optimistic revision refuses.
    assert_eq!(
        prepare(
            &issue_effects(root),
            StructuredActionV2::Issue,
            None,
            &root_bytes(terms.terms_id(), 8)
        ),
        Some(StructuredHotErrorV2::IdentityMismatch)
    );

    // A root bound to another Structured basis refuses.
    assert_eq!(
        prepare(
            &issue_effects(root),
            StructuredActionV2::Issue,
            None,
            &root_bytes(identity(0x6f), 7)
        ),
        Some(StructuredHotErrorV2::IdentityMismatch)
    );

    // Rent closure must be present exactly for retirement.
    assert_eq!(
        prepare(
            &issue_effects(root),
            StructuredActionV2::Issue,
            Some(rent_close(rent_credit)),
            &fixture.root_bytes
        ),
        Some(StructuredHotErrorV2::RentMismatch)
    );
    assert_eq!(
        prepare(
            &retirement_effects(root, rent_credit),
            StructuredActionV2::ZeroSupplyRetire,
            None,
            &fixture.root_bytes
        ),
        Some(StructuredHotErrorV2::RentMismatch)
    );

    // A nonzero receipt Mint supply refuses closure.
    let mut live_mint = retirement_effects(root, rent_credit);
    if let Some(effect) = live_mint.get_mut(2) {
        effect.pre_supply = 1;
        effect.post_supply = 1;
    }
    assert_eq!(
        prepare(
            &live_mint,
            StructuredActionV2::ZeroSupplyRetire,
            Some(rent_close(rent_credit)),
            &fixture.root_bytes
        ),
        Some(StructuredHotErrorV2::TokenMismatch)
    );
}

/// A wider, coprime basis, so that `quantity * c_i` is a different number for
/// every coordinate and no two rows can be swapped without changing an amount.
const WIDE_COEFFICIENTS: [u64; 3] = [2, 5, 11];

fn wide_shard_effect(
    index: u16,
    kind: StructuredHotTokenKindV2,
    basket: u64,
    authority: StructuredHotAccountRefV2,
    mints: &[[u8; 32]],
) -> StructuredHotTokenEffectV2 {
    let tag = u8::try_from(index).expect("coordinate tag");
    let holder = account_key(
        HOLDER_COORDINATE_BASE + index,
        identity(HOLDER_SHARD_BASE + tag),
    );
    let custody = account_key(
        CUSTODY_COORDINATE_BASE + index,
        identity(CUSTODY_SHARD_BASE + tag),
    );
    let (source, destination) = if kind == StructuredHotTokenKindV2::LockShards {
        (holder, custody)
    } else {
        (custody, holder)
    };
    StructuredHotTokenEffectV2 {
        kind,
        representation_coordinate: u32::from(index),
        token_program: account(TOKEN_PROGRAM_COORDINATE, TOKEN_PROGRAM),
        mint: account_key(
            SHARD_MINT_COORDINATE_BASE + index,
            mints.get(usize::from(index)).copied().expect("shard mint"),
        ),
        source: Some(source),
        destination: Some(destination),
        authority,
        amount: basket,
        pre_supply: 0,
        post_supply: 0,
        pre_source: 1_000,
        post_source: 1_000 - basket,
        pre_destination: 500,
        post_destination: 500 + basket,
    }
}

#[test]
fn every_locked_basket_is_the_quantity_times_its_own_coefficient() {
    // K_i = S * c_i, asserted as an identity rather than as three literals.
    // The fixtures elsewhere in this file write `QUANTITY * 3` by hand, which
    // cannot fail when the coefficient moves; these amounts are DERIVED, so a
    // coefficient change that the effect did not follow is a failing test.
    let shard_bytes = shard_terms_bytes(3, DENOMINATOR_FIXTURE);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&WIDE_COEFFICIENTS, DENOMINATOR_FIXTURE);
    let terms = structured_terms(&terms_bytes, shard);
    let root_state = root_bytes(digest(&terms_bytes), 7);
    let root = account(ROOT_COORDINATE, ROOT);
    let owner = account(OWNER_COORDINATE, OWNER);
    let mints = shard_mints(3);

    for (action, kind, authority) in [
        (
            StructuredActionV2::Issue,
            StructuredHotTokenKindV2::LockShards,
            owner,
        ),
        (
            StructuredActionV2::Unwrap,
            StructuredHotTokenKindV2::ReleaseShards,
            root,
        ),
        (
            StructuredActionV2::TerminalRedeem,
            StructuredHotTokenKindV2::ReleaseShards,
            root,
        ),
    ] {
        let receipt = if action == StructuredActionV2::Issue {
            StructuredHotTokenKindV2::MintReceipts
        } else {
            StructuredHotTokenKindV2::BurnReceipts
        };
        let mut effects = vec![receipt_effect(receipt, root)];
        let mut baskets = Vec::new();
        for (index, coefficient) in WIDE_COEFFICIENTS.iter().enumerate() {
            let basket = QUANTITY.checked_mul(*coefficient).expect("basket");
            baskets.push(basket);
            effects.push(wide_shard_effect(
                u16::try_from(index).expect("index"),
                kind,
                basket,
                authority,
                &mints,
            ));
        }
        // The three baskets must be pairwise distinct, or the per-coordinate
        // skew below would be provable by a swap and prove nothing.
        baskets.sort_unstable();
        let distinct = baskets.len();
        baskets.dedup();
        assert_eq!(baskets.len(), distinct);

        let candidate = StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
            request: request(action, terms),
            terms,
            shard_terms: shard,
            root_bytes: &root_state,
            root,
            token_effects: &effects,
            rent_close: None,
        })
        .expect("candidate accepts the exact basket");
        assert_eq!(candidate.token_effects().len(), 4);
        for (index, effect) in candidate.token_effects().iter().skip(1).enumerate() {
            let coefficient = WIDE_COEFFICIENTS.get(index).copied().expect("coefficient");
            assert_eq!(effect.amount, QUANTITY * coefficient);
        }

        // One atom off on ONE coordinate refuses, and the coordinate that moved
        // is the only thing that changed.
        for offset in 0..WIDE_COEFFICIENTS.len() {
            let mut skewed = effects.clone();
            let effect = skewed.get_mut(offset + 1).expect("shard effect");
            effect.amount += 1;
            effect.post_source -= 1;
            effect.post_destination += 1;
            assert_eq!(
                StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
                    request: request(action, terms),
                    terms,
                    shard_terms: shard,
                    root_bytes: &root_state,
                    root,
                    token_effects: &skewed,
                    rent_close: None,
                })
                .err(),
                Some(StructuredHotErrorV2::TokenMismatch),
                "{action:?} accepted a basket one atom off at coordinate {offset}"
            );
        }
    }
}

#[test]
fn a_structured_node_cannot_be_backed_by_its_own_receipts() {
    // The single backing edge points at the claim-shard layer and never past
    // it.  A shard row whose Mint is the receipt Mint would be a node backed by
    // itself, and the exact claim-shard terms are what refuse it: the receipt
    // Mint is not the shard Mint of any coordinate.
    let fixture = Fixture::new();
    let terms = fixture.terms();
    let root = account(ROOT_COORDINATE, ROOT);

    let mut self_backed = issue_effects(root);
    let effect = self_backed.get_mut(1).expect("first shard effect");
    effect.mint = account_key(SHARD_MINT_COORDINATE_BASE, terms.receipt_mint());
    assert_eq!(
        StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
            request: request(StructuredActionV2::Issue, terms),
            terms,
            shard_terms: fixture.shard(),
            root_bytes: &fixture.root_bytes,
            root,
            token_effects: &self_backed,
            rent_close: None,
        })
        .err(),
        Some(StructuredHotErrorV2::TokenMismatch)
    );

    // The mirror of the same confusion: the RECEIPT row naming a shard Mint.
    let mut shard_as_receipt = issue_effects(root);
    let effect = shard_as_receipt.get_mut(0).expect("receipt effect");
    effect.mint = account_key(
        RECEIPT_MINT_COORDINATE,
        shard_mints(2).first().copied().expect("shard mint"),
    );
    assert_eq!(
        StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
            request: request(StructuredActionV2::Issue, terms),
            terms,
            shard_terms: fixture.shard(),
            root_bytes: &fixture.root_bytes,
            root,
            token_effects: &shard_as_receipt,
            rent_close: None,
        })
        .err(),
        Some(StructuredHotErrorV2::TokenMismatch)
    );
}
