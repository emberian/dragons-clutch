//! Chain-derived planning evidence, and its exact agreement with the onchain
//! candidate the physical adapter will execute.

mod support;

use dclutch_structured_v2_contract::{
    STRUCTURED_ACCOUNT_ACTOR_V2, STRUCTURED_ACCOUNT_RECEIPT_MINT_V2,
    STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2, STRUCTURED_ACCOUNT_RENT_CREDIT_V2,
    STRUCTURED_ACCOUNT_RENT_PROGRAM_V2, STRUCTURED_ACCOUNT_ROOT_V2,
    STRUCTURED_ACCOUNT_TOKEN_PROGRAM_V2, StructuredActionV2, StructuredFrameSpecV2,
    StructuredHotAccountRefV2, StructuredHotCandidateInputV2, StructuredHotCandidateV2,
    StructuredHotRentCloseV2, StructuredHotTokenEffectV2, StructuredHotTokenKindV2,
};
use dclutch_structured_v2_kernel::{
    STRUCTURED_NO_COORDINATE_V2, StructuredCoordinateObservationV2, StructuredPhaseV2,
    StructuredTermsV2,
};
use dclutch_structured_v2_operator::{
    Error, StructuredActionObservationV2, StructuredHotProfileV2, StructuredIntentV2,
    StructuredRequestContextV2, StructuredShardAccountObservationV2,
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

/// Both fixture coefficients are nonzero, so the backed width IS the
/// representation width and a backed index is its own coordinate.
const BACKED_COORDINATES: usize = 2;

fn frame() -> StructuredFrameSpecV2 {
    StructuredFrameSpecV2::new(BACKED_COORDINATES).expect("frame")
}

fn coordinate(index: usize) -> u16 {
    u16::try_from(index).expect("coordinate")
}

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

/// The AccountProfile expansion the FRAME CONTRACT specifies, at exactly the
/// frame's own width.
///
/// This function used to invent its own layout -- token program at 0, root at
/// 1, and the shard mints, holder accounts and custody accounts in three
/// separate blocks based at 5, 10 and 20, indexed by representation coordinate.
/// The frame contract that landed later says something different in every one
/// of those coordinates, and interleaves the three shard accounts as one triple
/// per BACKED coordinate.  Both were account-layout authorities and nothing
/// made them agree, so the operator's only executable evidence was agreement
/// with a layout the adapter would never reconstruct.  The invented constants
/// are gone; every coordinate below is asked of `frame.rs`.
fn profile_keys(action: StructuredActionV2) -> Vec<[u8; 32]> {
    let spec = frame();
    let width = spec.account_count().expect("account count");
    // Distinct filler everywhere, so any coordinate this fixture does not name
    // on purpose still refuses to alias one that it does.
    let mut keys: Vec<[u8; 32]> = (0..width)
        .map(|index| identity(0x80 + u8::try_from(index).expect("index")))
        .collect();
    let mut assign = |index: usize, key: [u8; 32]| {
        *keys.get_mut(index).expect("frame coordinate") = key;
    };
    assign(STRUCTURED_ACCOUNT_ACTOR_V2, identity(OWNER));
    assign(STRUCTURED_ACCOUNT_ROOT_V2, identity(ROOT));
    assign(STRUCTURED_ACCOUNT_TOKEN_PROGRAM_V2, identity(TOKEN_PROGRAM));
    assign(STRUCTURED_ACCOUNT_RECEIPT_MINT_V2, identity(RECEIPT_MINT));
    assign(
        STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2,
        receipt_account_key(action),
    );
    assign(STRUCTURED_ACCOUNT_RENT_CREDIT_V2, identity(RENT_CREDIT));
    assign(STRUCTURED_ACCOUNT_RENT_PROGRAM_V2, identity(RENT_PROGRAM));
    let mints = shard_mints(BACKED_COORDINATES);
    for index in 0..BACKED_COORDINATES {
        let shard = u8::try_from(index).expect("index");
        assign(
            spec.shard_mint(index).expect("shard mint"),
            mints.get(index).copied().expect("mint"),
        );
        assign(
            spec.actor_shard(index).expect("actor shard"),
            identity(HOLDER_SHARD_BASE + shard),
        );
        assign(
            spec.custody_shard(index).expect("custody shard"),
            identity(CUSTODY_SHARD_BASE + shard),
        );
    }
    keys
}

/// Output storage the lowering must fully overwrite; every field is wrong on
/// purpose, so a coordinate the projection skipped cannot pass as lowered.
fn inert_effect() -> StructuredHotTokenEffectV2 {
    let placeholder = StructuredHotAccountRefV2::new(0, identity(TOKEN_PROGRAM)).expect("ref");
    StructuredHotTokenEffectV2 {
        kind: StructuredHotTokenKindV2::MintReceipts,
        representation_coordinate: STRUCTURED_NO_COORDINATE_V2,
        token_program: placeholder,
        mint: placeholder,
        source: None,
        destination: None,
        authority: placeholder,
        amount: 0,
        pre_supply: 0,
        post_supply: 0,
        pre_source: 0,
        post_source: 0,
        pre_destination: 0,
        post_destination: 0,
    }
}

fn receipt_account_key(action: StructuredActionV2) -> [u8; 32] {
    if action == StructuredActionV2::Issue {
        identity(RECEIPT_DESTINATION)
    } else {
        identity(RECEIPT_SOURCE)
    }
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

    let keys = profile_keys(action);
    let profile = StructuredHotProfileV2::new(&keys).expect("profile");
    let mut lowered = vec![inert_effect(); plan.effects.len()];
    // The frame, not a hand-written table, decides where every account sits.
    lower_structured_hot_effects_v2(profile, &plan, frame(), &mut lowered).expect("lower effects");

    let rent_close: Option<StructuredHotRentCloseV2> =
        if action == StructuredActionV2::ZeroSupplyRetire {
            Some(
                lower_structured_hot_rent_close_v2(
                    profile,
                    identity(RENT_PROGRAM),
                    identity(RENT_CREDIT),
                    identity(0x99),
                    40,
                )
                .expect("rent close"),
            )
        } else {
            None
        };

    let root_ref =
        StructuredHotAccountRefV2::new(coordinate(STRUCTURED_ACCOUNT_ROOT_V2), identity(ROOT))
            .expect("root ref");
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

/// The layout this test file used to invent, kept only so that reverting to it
/// is a failing test rather than a silent disagreement with the adapter.
fn superseded_block_layout_keys() -> Vec<[u8; 32]> {
    const BLOCK_WIDTH: usize = 32;
    const BLOCK_TOKEN_PROGRAM: usize = 0;
    const BLOCK_ROOT: usize = 1;
    const BLOCK_OWNER: usize = 2;
    const BLOCK_RECEIPT_MINT: usize = 3;
    const BLOCK_RECEIPT_ACCOUNT: usize = 4;
    const BLOCK_SHARD_MINT_BASE: usize = 5;
    const BLOCK_HOLDER_BASE: usize = 10;
    const BLOCK_CUSTODY_BASE: usize = 20;
    let mut keys: Vec<[u8; 32]> = (0..BLOCK_WIDTH)
        .map(|index| identity(0x80 + u8::try_from(index).expect("index")))
        .collect();
    let mut assign = |index: usize, key: [u8; 32]| {
        *keys.get_mut(index).expect("block coordinate") = key;
    };
    assign(BLOCK_TOKEN_PROGRAM, identity(TOKEN_PROGRAM));
    assign(BLOCK_ROOT, identity(ROOT));
    assign(BLOCK_OWNER, identity(OWNER));
    assign(BLOCK_RECEIPT_MINT, identity(RECEIPT_MINT));
    assign(BLOCK_RECEIPT_ACCOUNT, identity(RECEIPT_DESTINATION));
    let mints = shard_mints(BACKED_COORDINATES);
    for index in 0..BACKED_COORDINATES {
        let shard = u8::try_from(index).expect("index");
        assign(
            BLOCK_SHARD_MINT_BASE + index,
            mints.get(index).copied().expect("mint"),
        );
        assign(
            BLOCK_HOLDER_BASE + index,
            identity(HOLDER_SHARD_BASE + shard),
        );
        assign(
            BLOCK_CUSTODY_BASE + index,
            identity(CUSTODY_SHARD_BASE + shard),
        );
    }
    keys
}

#[test]
fn the_superseded_block_layout_no_longer_expands() {
    let shard_bytes = shard_terms_bytes(BACKED_COORDINATES, DENOMINATOR_FIXTURE);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&COEFFICIENTS, DENOMINATOR_FIXTURE);
    let terms = structured_terms(&terms_bytes, shard);
    let observed = Observed::new(SUPPLY, &[0, 0]);
    let plan = plan_structured_action_v2(
        terms,
        shard,
        context(terms),
        StructuredIntentV2 {
            action: StructuredActionV2::Issue,
            receipt_atoms: 4,
        },
        observed.observation(StructuredPhaseV2::Open, SUPPLY, StructuredActionV2::Issue),
    )
    .expect("action plan");
    let keys = superseded_block_layout_keys();
    let profile = StructuredHotProfileV2::new(&keys).expect("profile");
    let mut lowered = vec![inert_effect(); plan.effects.len()];
    // Every account of this expansion is a real fixture identity; it is only
    // at the WRONG coordinate.  The frame is what refuses it.
    assert_eq!(
        lower_structured_hot_effects_v2(profile, &plan, frame(), &mut lowered),
        Err(Error::AccountFrame)
    );
}

fn shard_accounts_at(
    base: u8,
    coordinates: &[u32],
    amounts: &[u64],
) -> Vec<StructuredShardAccountObservationV2> {
    coordinates
        .iter()
        .zip(amounts)
        .enumerate()
        .map(
            |(backed, (coordinate, amount))| StructuredShardAccountObservationV2 {
                representation_coordinate: *coordinate,
                account: identity(base + u8::try_from(backed).expect("backed")),
                amount: *amount,
            },
        )
        .collect()
}

#[test]
fn a_zero_coefficient_row_contributes_no_accounts_and_closes_the_gap() {
    // The sharpest case for the two indexing rules: a middle row that moves
    // nothing.  A layout keyed by REPRESENTATION coordinate would leave a hole
    // at coordinate 1 and put the second backed row's accounts at index 2; the
    // frame closes the gap, so the second backed row sits in triple 1.
    const GAPPED: [u64; 3] = [1, 0, 3];
    let shard_bytes = shard_terms_bytes(3, DENOMINATOR_FIXTURE);
    let shard = shard_terms(&shard_bytes);
    let terms_bytes = structured_terms_bytes(&GAPPED, DENOMINATOR_FIXTURE);
    let terms = structured_terms(&terms_bytes, shard);
    assert_eq!(terms.representation_width(), 3);

    let rows = exact_rows(&GAPPED, SUPPLY, &[0, 0, 0]);
    let holder = shard_accounts_at(HOLDER_SHARD_BASE, &[0, 2], &[1_000, 1_000]);
    let custody = shard_accounts_at(CUSTODY_SHARD_BASE, &[0, 2], &[SUPPLY, SUPPLY * 3]);
    let base = Observed::new(SUPPLY, &[0, 0]);
    let mut observation =
        base.observation(StructuredPhaseV2::Open, SUPPLY, StructuredActionV2::Issue);
    observation.rows = &rows;
    observation.holder_shard_accounts = &holder;
    observation.custody_shard_accounts = &custody;

    let plan = plan_structured_action_v2(
        terms,
        shard,
        context(terms),
        StructuredIntentV2 {
            action: StructuredActionV2::Issue,
            receipt_atoms: 4,
        },
        observation,
    )
    .expect("action plan");
    // Three-wide terms, two backed rows: one receipt effect plus two shards.
    assert_eq!(plan.effects.len(), 3);
    assert_eq!(
        plan.effects
            .iter()
            .skip(1)
            .map(|effect| effect.representation_coordinate)
            .collect::<Vec<_>>(),
        vec![0, 2],
    );

    // The frame is sized by the BACKED count, not the representation width, so
    // the zero row contributes no accounts at all.
    let spec = StructuredFrameSpecV2::new(2).expect("frame");
    assert_eq!(spec.account_count(), Ok(29));
    let mints = shard_mints(3);
    let mut keys: Vec<[u8; 32]> = (0..spec.account_count().expect("count"))
        .map(|index| identity(0x80 + u8::try_from(index).expect("index")))
        .collect();
    let mut assign = |index: usize, key: [u8; 32]| {
        *keys.get_mut(index).expect("frame coordinate") = key;
    };
    assign(STRUCTURED_ACCOUNT_ACTOR_V2, identity(OWNER));
    assign(STRUCTURED_ACCOUNT_ROOT_V2, identity(ROOT));
    assign(STRUCTURED_ACCOUNT_TOKEN_PROGRAM_V2, identity(TOKEN_PROGRAM));
    assign(STRUCTURED_ACCOUNT_RECEIPT_MINT_V2, identity(RECEIPT_MINT));
    assign(
        STRUCTURED_ACCOUNT_RECEIPT_TOKEN_V2,
        identity(RECEIPT_DESTINATION),
    );
    // Backed row 1 is representation coordinate 2, and its Mint goes in the
    // SECOND triple -- the gap is closed, not preserved.
    for (backed, coordinate) in [0_usize, 2].into_iter().enumerate() {
        assign(
            spec.shard_mint(backed).expect("mint"),
            mints.get(coordinate).copied().expect("mint"),
        );
        assign(
            spec.actor_shard(backed).expect("actor"),
            identity(HOLDER_SHARD_BASE + u8::try_from(backed).expect("backed")),
        );
        assign(
            spec.custody_shard(backed).expect("custody"),
            identity(CUSTODY_SHARD_BASE + u8::try_from(backed).expect("backed")),
        );
    }
    let profile = StructuredHotProfileV2::new(&keys).expect("profile");
    let mut lowered = vec![inert_effect(); plan.effects.len()];
    lower_structured_hot_effects_v2(profile, &plan, spec, &mut lowered).expect("lower effects");

    let root_ref =
        StructuredHotAccountRefV2::new(coordinate(STRUCTURED_ACCOUNT_ROOT_V2), identity(ROOT))
            .expect("root ref");
    let root_bytes = root_state(terms.terms_id(), REVISION);
    let candidate = StructuredHotCandidateV2::prepare(StructuredHotCandidateInputV2 {
        request: plan.request,
        terms,
        shard_terms: shard,
        root_bytes: &root_bytes,
        root: root_ref,
        token_effects: &lowered,
        rent_close: None,
    })
    .expect("candidate accepts the gapped plan");
    assert_eq!(candidate.action(), StructuredActionV2::Issue);
    assert_eq!(candidate.token_effects().len(), 3);
}
