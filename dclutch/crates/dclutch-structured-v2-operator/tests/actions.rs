//! Chain-derived planning evidence, and its exact agreement with the onchain
//! candidate the physical adapter will execute.

mod support;

use dclutch_structured_v2_contract::{
    StructuredActionV2, StructuredHotAccountRefV2, StructuredHotCandidateInputV2,
    StructuredHotCandidateV2, StructuredHotRentCloseV2, StructuredHotTokenEffectV2,
    StructuredHotTokenKindV2,
};
use dclutch_structured_v2_kernel::{
    STRUCTURED_NO_COORDINATE_V2, StructuredCoordinateObservationV2, StructuredPhaseV2,
    StructuredTermsV2,
};
use dclutch_structured_v2_operator::{
    Error, StructuredActionObservationV2, StructuredHotEffectCoordinatesV2, StructuredHotProfileV2,
    StructuredIntentV2, StructuredRequestContextV2, StructuredShardAccountObservationV2,
    lower_structured_hot_effects_v2, lower_structured_hot_rent_close_v2, plan_structured_action_v2,
};
use support::{
    CUSTODY_SHARD_BASE, DENOMINATOR_FIXTURE, HOLDER_SHARD_BASE, MARKET, OWNER, PRODUCT_RECORD,
    RECEIPT_DESTINATION, RECEIPT_MINT, RECEIPT_SOURCE, RECEIPT_TOKEN_BEHAVIOR, RELEASE_SET,
    RENT_CREDIT, RENT_PROGRAM, RESULT_DOMAIN, ROOT, SHARD_EXPOSURE, TERMINAL_DIGEST, TOKEN_PROGRAM,
    digest, exact_rows, identity, shard_mints, shard_terms, shard_terms_bytes, structured_terms,
    structured_terms_bytes,
};

const COEFFICIENTS: [u64; 2] = [1, 3];
const SUPPLY: u64 = 10;
const REVISION: u64 = 7;

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
const PROFILE_WIDTH: usize = 32;

fn context(terms: StructuredTermsV2<'_>) -> StructuredRequestContextV2 {
    StructuredRequestContextV2 {
        release_set: identity(RELEASE_SET),
        market: identity(MARKET),
        product_record: identity(PRODUCT_RECORD),
        result_domain: identity(RESULT_DOMAIN),
        terms: terms.terms_id(),
        token_behavior: identity(RECEIPT_TOKEN_BEHAVIOR),
        shard_terms: terms.shard_terms(),
        shard_exposure: identity(SHARD_EXPOSURE),
    }
}

fn shard_accounts(base: u8, amounts: &[u64]) -> Vec<StructuredShardAccountObservationV2> {
    amounts
        .iter()
        .enumerate()
        .map(|(index, amount)| StructuredShardAccountObservationV2 {
            representation_coordinate: u32::try_from(index).expect("coordinate"),
            account: identity(base + u8::try_from(index).expect("index")),
            amount: *amount,
        })
        .collect()
}

struct Observed {
    rows: Vec<StructuredCoordinateObservationV2>,
    holder: Vec<StructuredShardAccountObservationV2>,
    custody: Vec<StructuredShardAccountObservationV2>,
}

impl Observed {
    fn new(supply: u64, payouts: &[u64]) -> Self {
        Self {
            rows: exact_rows(&COEFFICIENTS, supply, payouts),
            holder: shard_accounts(HOLDER_SHARD_BASE, &[1_000, 1_000]),
            custody: shard_accounts(CUSTODY_SHARD_BASE, &[supply, supply * 3]),
        }
    }

    fn observation(
        &self,
        phase: StructuredPhaseV2,
        supply: u64,
        action: StructuredActionV2,
    ) -> StructuredActionObservationV2<'_> {
        let carries = action != StructuredActionV2::ZeroSupplyRetire;
        StructuredActionObservationV2 {
            finalized: true,
            phase,
            terminal_digest: if matches!(
                action,
                StructuredActionV2::TerminalRedeem | StructuredActionV2::ZeroSupplyRetire
            ) {
                identity(TERMINAL_DIGEST)
            } else {
                [0; 32]
            },
            revision: REVISION,
            receipt_supply: supply,
            rows: &self.rows,
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
            actor_receipts: supply,
            token_program: identity(TOKEN_PROGRAM),
            root: identity(ROOT),
            holder_shard_accounts: &self.holder,
            custody_shard_accounts: &self.custody,
            rent_credit: if carries {
                [0; 32]
            } else {
                identity(RENT_CREDIT)
            },
        }
    }
}

/// Dense AccountProfile expansion covering every coordinate the fixtures use.
fn profile_keys() -> Vec<[u8; 32]> {
    let mut keys: Vec<[u8; 32]> = (0..PROFILE_WIDTH)
        .map(|index| identity(0x80 + u8::try_from(index).expect("index")))
        .collect();
    let mints = shard_mints(2);
    let assign = |keys: &mut Vec<[u8; 32]>, coordinate: u16, key: [u8; 32]| {
        if let Some(slot) = keys.get_mut(usize::from(coordinate)) {
            *slot = key;
        }
    };
    assign(&mut keys, TOKEN_PROGRAM_COORDINATE, identity(TOKEN_PROGRAM));
    assign(&mut keys, ROOT_COORDINATE, identity(ROOT));
    assign(&mut keys, OWNER_COORDINATE, identity(OWNER));
    assign(&mut keys, RECEIPT_MINT_COORDINATE, identity(RECEIPT_MINT));
    assign(&mut keys, RENT_PROGRAM_COORDINATE, identity(RENT_PROGRAM));
    assign(&mut keys, RENT_CREDIT_COORDINATE, identity(RENT_CREDIT));
    for index in 0..2_u16 {
        assign(
            &mut keys,
            SHARD_MINT_COORDINATE_BASE + index,
            mints.get(usize::from(index)).copied().unwrap_or_default(),
        );
        assign(
            &mut keys,
            HOLDER_COORDINATE_BASE + index,
            identity(HOLDER_SHARD_BASE + u8::try_from(index).expect("index")),
        );
        assign(
            &mut keys,
            CUSTODY_COORDINATE_BASE + index,
            identity(CUSTODY_SHARD_BASE + u8::try_from(index).expect("index")),
        );
    }
    keys
}

fn receipt_account_key(action: StructuredActionV2) -> [u8; 32] {
    if action == StructuredActionV2::Issue {
        identity(RECEIPT_DESTINATION)
    } else {
        identity(RECEIPT_SOURCE)
    }
}

fn effect_coordinates(
    effects: &[dclutch_structured_v2_operator::StructuredTokenEffectPlanV2],
) -> Vec<StructuredHotEffectCoordinatesV2> {
    effects
        .iter()
        .map(|effect| {
            let shard = effect.representation_coordinate != STRUCTURED_NO_COORDINATE_V2;
            let index = if shard {
                u16::try_from(effect.representation_coordinate).expect("coordinate")
            } else {
                0
            };
            StructuredHotEffectCoordinatesV2 {
                token_program: TOKEN_PROGRAM_COORDINATE,
                mint: if shard {
                    SHARD_MINT_COORDINATE_BASE + index
                } else {
                    RECEIPT_MINT_COORDINATE
                },
                source: effect.source.map(|_| {
                    if !shard {
                        RECEIPT_ACCOUNT_COORDINATE
                    } else if effect.kind == StructuredHotTokenKindV2::LockShards {
                        HOLDER_COORDINATE_BASE + index
                    } else {
                        CUSTODY_COORDINATE_BASE + index
                    }
                }),
                destination: effect.destination.map(|_| {
                    if !shard {
                        if effect.kind == StructuredHotTokenKindV2::CloseReceiptMint {
                            RENT_CREDIT_COORDINATE
                        } else {
                            RECEIPT_ACCOUNT_COORDINATE
                        }
                    } else if effect.kind == StructuredHotTokenKindV2::CloseCustody {
                        RENT_CREDIT_COORDINATE
                    } else if effect.kind == StructuredHotTokenKindV2::LockShards {
                        CUSTODY_COORDINATE_BASE + index
                    } else {
                        HOLDER_COORDINATE_BASE + index
                    }
                }),
                authority: if effect.kind == StructuredHotTokenKindV2::LockShards {
                    OWNER_COORDINATE
                } else {
                    ROOT_COORDINATE
                },
            }
        })
        .collect()
}

fn round_trip(action: StructuredActionV2, phase: StructuredPhaseV2, supply: u64, payouts: &[u64]) {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR_FIXTURE);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR_FIXTURE);
    let terms = structured_terms(&terms_bytes, shard);
    let observed = Observed::new(supply, payouts);
    let quantity = if action == StructuredActionV2::ZeroSupplyRetire {
        0
    } else {
        4
    };
    let plan = plan_structured_action_v2(
        terms,
        shard,
        context(terms),
        StructuredIntentV2 {
            action,
            receipt_atoms: quantity,
        },
        observed.observation(phase, supply, action),
    )
    .expect("action plan");

    let keys = profile_keys();
    let mut keys = keys;
    if let Some(slot) = keys.get_mut(usize::from(RECEIPT_ACCOUNT_COORDINATE)) {
        *slot = receipt_account_key(action);
    }
    let profile = StructuredHotProfileV2::new(&keys).expect("profile");
    let coordinates = effect_coordinates(&plan.effects);
    let mut lowered = vec![
        StructuredHotTokenEffectV2 {
            kind: StructuredHotTokenKindV2::MintReceipts,
            representation_coordinate: STRUCTURED_NO_COORDINATE_V2,
            token_program: StructuredHotAccountRefV2::new(0, identity(TOKEN_PROGRAM)).expect("ref"),
            mint: StructuredHotAccountRefV2::new(0, identity(TOKEN_PROGRAM)).expect("ref"),
            source: None,
            destination: None,
            authority: StructuredHotAccountRefV2::new(0, identity(TOKEN_PROGRAM)).expect("ref"),
            amount: 0,
            pre_supply: 0,
            post_supply: 0,
            pre_source: 0,
            post_source: 0,
            pre_destination: 0,
            post_destination: 0,
        };
        plan.effects.len()
    ];
    lower_structured_hot_effects_v2(profile, &plan, &coordinates, &mut lowered)
        .expect("lower effects");

    let rent_close: Option<StructuredHotRentCloseV2> =
        if action == StructuredActionV2::ZeroSupplyRetire {
            Some(
                lower_structured_hot_rent_close_v2(
                    profile,
                    identity(RENT_PROGRAM),
                    identity(RENT_CREDIT),
                    identity(0x99),
                    RENT_PROGRAM_COORDINATE,
                    RENT_CREDIT_COORDINATE,
                    40,
                )
                .expect("rent close"),
            )
        } else {
            None
        };

    let root_ref =
        StructuredHotAccountRefV2::new(ROOT_COORDINATE, identity(ROOT)).expect("root ref");
    let root_bytes = root_state(terms.terms_id(), REVISION);
    let candidate = StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
        request: plan.request,
        terms,
        shard_terms: shard,
        root_bytes: &root_bytes,
        root: root_ref,
        token_effects: &lowered,
        rent_close,
    })
    .expect("candidate accepts the operator plan");
    assert_eq!(candidate.action(), action);
    assert_eq!(candidate.pre_revision(), REVISION);
    assert_eq!(candidate.post_revision(), plan.post_revision);
}

fn root_state(terms_id: [u8; 32], revision: u64) -> Vec<u8> {
    use dclutch_structured_v2_contract::{StructuredRootInputV2, StructuredRootV2};
    StructuredRootV2::new(StructuredRootInputV2 {
        bump: 254,
        terms: terms_id,
        market: identity(MARKET),
        rent_beneficiary: identity(0x22),
        revision,
        historical_rent_principal: 2_039_280,
    })
    .expect("root")
    .to_bytes()
    .to_vec()
}

#[test]
fn issue_plan_is_accepted_by_the_onchain_candidate() {
    round_trip(
        StructuredActionV2::Issue,
        StructuredPhaseV2::Open,
        SUPPLY,
        &[0, 0],
    );
}

#[test]
fn unwrap_plan_is_accepted_by_the_onchain_candidate() {
    round_trip(
        StructuredActionV2::Unwrap,
        StructuredPhaseV2::Open,
        SUPPLY,
        &[0, 0],
    );
}

#[test]
fn terminal_redeem_plan_is_accepted_by_the_onchain_candidate() {
    round_trip(
        StructuredActionV2::TerminalRedeem,
        StructuredPhaseV2::Terminal,
        SUPPLY,
        &[0, 5],
    );
}

#[test]
fn retire_plan_is_accepted_by_the_onchain_candidate() {
    round_trip(
        StructuredActionV2::ZeroSupplyRetire,
        StructuredPhaseV2::Terminal,
        0,
        &[0, 5],
    );
}

#[test]
fn terminal_plan_carries_the_exact_settlement_projection() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR_FIXTURE);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR_FIXTURE);
    let terms = structured_terms(&terms_bytes, shard);
    let observed = Observed::new(SUPPLY, &[0, 5]);
    let plan = plan_structured_action_v2(
        terms,
        shard,
        context(terms),
        StructuredIntentV2 {
            action: StructuredActionV2::TerminalRedeem,
            receipt_atoms: SUPPLY,
        },
        observed.observation(
            StructuredPhaseV2::Terminal,
            SUPPLY,
            StructuredActionV2::TerminalRedeem,
        ),
    )
    .expect("terminal plan");
    assert_eq!(plan.total_shard_atoms, 40);
    assert_eq!(plan.total_collateral_atoms, 35);
    assert_eq!(plan.post_receipt_supply, 0);
    let losing = plan.settlement.first().copied().expect("row");
    assert_eq!(losing.whole_claims, 2);
    assert_eq!(losing.change_shards, 2);
    assert_eq!(losing.collateral_atoms, 0);
    let winning = plan.settlement.get(1).copied().expect("row");
    assert_eq!(winning.whole_claims, 7);
    assert_eq!(winning.change_shards, 2);
    assert_eq!(winning.collateral_atoms, 35);
    assert_eq!(
        plan.request.input().terminal_digest,
        identity(TERMINAL_DIGEST)
    );
}

#[test]
fn operator_refuses_unfinalized_and_inconsistent_observations() {
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR_FIXTURE);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR_FIXTURE);
    let terms = structured_terms(&terms_bytes, shard);
    let observed = Observed::new(SUPPLY, &[0, 0]);
    let intent = StructuredIntentV2 {
        action: StructuredActionV2::Issue,
        receipt_atoms: 4,
    };

    let mut unfinalized =
        observed.observation(StructuredPhaseV2::Open, SUPPLY, StructuredActionV2::Issue);
    unfinalized.finalized = false;
    assert_eq!(
        plan_structured_action_v2(terms, shard, context(terms), intent, unfinalized),
        Err(Error::ChainObservation)
    );

    // Terminal evidence on an open Market refuses.
    let mut early_terminal =
        observed.observation(StructuredPhaseV2::Open, SUPPLY, StructuredActionV2::Issue);
    early_terminal.terminal_digest = identity(TERMINAL_DIGEST);
    assert_eq!(
        plan_structured_action_v2(terms, shard, context(terms), intent, early_terminal),
        Err(Error::ChainObservation)
    );

    // A substituted Token program refuses.
    let mut wrong_program =
        observed.observation(StructuredPhaseV2::Open, SUPPLY, StructuredActionV2::Issue);
    wrong_program.token_program = identity(0x7d);
    assert_eq!(
        plan_structured_action_v2(terms, shard, context(terms), intent, wrong_program),
        Err(Error::ChainObservation)
    );

    // A context that disagrees with the immutable terms refuses.
    let mut wrong_context = context(terms);
    wrong_context.shard_exposure = identity(0x7c);
    assert_eq!(
        plan_structured_action_v2(
            terms,
            shard,
            wrong_context,
            intent,
            observed.observation(StructuredPhaseV2::Open, SUPPLY, StructuredActionV2::Issue),
        ),
        Err(Error::Terms)
    );

    // A shard-account list of the wrong width refuses.
    let short_custody = shard_accounts(CUSTODY_SHARD_BASE, &[SUPPLY]);
    let mut short =
        observed.observation(StructuredPhaseV2::Open, SUPPLY, StructuredActionV2::Issue);
    short.custody_shard_accounts = &short_custody;
    assert_eq!(
        plan_structured_action_v2(terms, shard, context(terms), intent, short),
        Err(Error::ChainObservation)
    );
}

#[test]
fn structured_terms_digest_is_stable() {
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR_FIXTURE);
    let shard_bytes = shard_terms_bytes(2, DENOMINATOR_FIXTURE);
    let shard = shard_terms(&shard_bytes);
    let terms = structured_terms(&terms_bytes, shard);
    assert_eq!(terms.terms_id(), digest(&terms_bytes));
    assert_eq!(terms.shard_terms(), digest(&shard_bytes));
}
