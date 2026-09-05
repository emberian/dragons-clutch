//! Ordered Fractional retirement: begin the walk, advance it, close it.
//!
//! `Begin` creates the cursor the walk advances, once, on a Market that has
//! gone terminal. `RetireCoordinate` closes the zero native Position/admission
//! pair, then invokes Token-2022 to close the matching zero-supply Mint, and
//! commits the cursor last. `Finish` closes the cursor after -- and only
//! after -- every coordinate has advanced, and settles its lamports to the
//! Market's own RentCredit. The Trading-owned root is authenticated and
//! borrowed only through a readonly Claims view; its propagated signature is
//! used solely as the Token-2022 MintCloseAuthority. Any late CPI or cursor
//! failure rolls every earlier mutation back at the SVM instruction boundary.
//!
//! **Nothing here is privileged.** All three acts are permissionless, because
//! all three are fully determined by state that is authenticated before they
//! run: the cursor's content comes from the finalized terms and the root, the
//! coordinate order comes from the cursor, and the rent beneficiary comes from
//! the root. A route that additionally demanded a signature would hand
//! whoever holds it the power to strand every shard holder's collateral behind
//! a walk nobody else may crank -- the hostage-taking this family already
//! refuses one phase earlier in `claim_check_compaction_v1`.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_claims::{
    liability_basis_state_v2::{
        LiabilityBasisMarketSeedsV2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
    },
    protocol_position_v2::{
        ProtocolPositionActionV2, ProtocolPositionAdmissionV2, ProtocolPositionOwnerKindV2,
        ProtocolPositionPresenceV2, ProtocolPositionRequestV2,
    },
};
use dclutch_claims::fractional::{
    FRACTIONAL_RETIREMENT_BEGIN_ACCOUNT_COUNT_V3,
    FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3,
    FRACTIONAL_RETIREMENT_COORDINATE_RECEIPT_BYTES_V3, FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3,
    FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3, FRACTIONAL_RETIREMENT_FINISH_ACCOUNT_COUNT_V3,
    FRACTIONAL_RETIREMENT_LIFECYCLE_RECEIPT_BYTES_V3, FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3,
    FractionalRetireCoordinateObservationV3, FractionalRetirementActionV3,
    FractionalRetirementCoordinateReceiptV3, FractionalRetirementCursorInputV3,
    FractionalRetirementCursorV3, FractionalRetirementLifecycleObservationV3,
    FractionalRetirementLifecycleReceiptV3, FractionalRetirementRequestV3,
    decode_fractional_capability_root_v4,
};
use dclutch_claims::fractional_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FRACTIONAL_SELECTION_CONFIG_BYTES_V1,
    FractionalExposureTermsAdmissionV2, FractionalExposureTermsV2,
    encode_fractional_selection_config_v1, fractional_selection_config_from_terms_v1,
};
use dclutch_market::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_sha256_adapter::digest;
use dclutch_custody::token_svm::{
    TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2, Token2022BehaviorProfileV2, TokenBehaviorSelectionV2,
};
use solana_program::{
    account_info::AccountInfo,
    instruction::Instruction,
    program::{invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;
use solana_system_interface::instruction::{allocate, assign, transfer};
use spl_token_2022_interface::instruction as token_instruction;

use crate::{
    ClaimsSbfError,
    market_admission_v1::CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1,
    protocol_position_v2::{
        AuthenticatedProtocolPositionCloseParentV2, LifecycleRentCreditIdentityV2,
        PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2, authenticate_rent_credit,
        execute_parent_authenticated_close,
    },
    rational_representation_v2::authenticate_finalized_rational_record,
};

const AUTHORITY: usize = 0;
const MARKET: usize = 1;
const POSITION: usize = 2;
const ADMISSION: usize = 3;
const RENT: usize = 4;
const REGISTRY: usize = 7;
const TRADING_PROGRAM: usize = 8;
const CLAIMS_PROGRAM: usize = 10;
const ROOT: usize = 12;
const RENT_CREDIT: usize = 13;
const RENT_PROGRAM: usize = 14;
const CURSOR: usize = 15;
const TERMS_RAW: usize = 16;
const TERMS_STAGING: usize = 17;
const TOKEN_BEHAVIOR_RAW: usize = 18;
const TOKEN_BEHAVIOR_STAGING: usize = 19;
const SHARD_MINT: usize = 20;
const TOKEN_PROGRAM: usize = 21;

// The begin frame. It shares no index with the coordinate frame on purpose:
// the coordinate frame's first fifteen slots are the borrowed
// ProtocolPosition close frame, and begin closes no Position, so reusing that
// prefix would mean carrying five accounts it must never touch.
const BEGIN_PAYER: usize = 0;
const BEGIN_MARKET: usize = 1;
const BEGIN_CORE_MARKET: usize = 2;
const BEGIN_CORE_PROGRAM: usize = 3;
const BEGIN_REGISTRY: usize = 4;
const BEGIN_RENT: usize = 5;
const BEGIN_TRADING_PROGRAM: usize = 6;
const BEGIN_CLAIMS_PROGRAM: usize = 7;
const BEGIN_ROOT: usize = 8;
const BEGIN_RENT_CREDIT: usize = 9;
const BEGIN_CURSOR: usize = 10;
const BEGIN_TERMS_RAW: usize = 11;
const BEGIN_TERMS_STAGING: usize = 12;
const BEGIN_TOKEN_BEHAVIOR_RAW: usize = 13;
const BEGIN_TOKEN_BEHAVIOR_STAGING: usize = 14;
const BEGIN_SYSTEM: usize = 15;

// The finish frame.
const FINISH_MARKET: usize = 0;
const FINISH_REGISTRY: usize = 1;
const FINISH_RENT: usize = 2;
const FINISH_TRADING_PROGRAM: usize = 3;
const FINISH_CLAIMS_PROGRAM: usize = 4;
const FINISH_ROOT: usize = 5;
const FINISH_RENT_CREDIT: usize = 6;
const FINISH_RENT_PROGRAM: usize = 7;
const FINISH_CURSOR: usize = 8;
const FINISH_TERMS_RAW: usize = 9;
const FINISH_TERMS_STAGING: usize = 10;
const FINISH_TOKEN_BEHAVIOR_RAW: usize = 11;
const FINISH_TOKEN_BEHAVIOR_STAGING: usize = 12;

/// Execute one exact ordered-retirement act.
#[inline(never)]
pub(super) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    if instruction_data.len() != FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3 {
        return Err(ClaimsSbfError::Instruction.into());
    }
    let request = FractionalRetirementRequestV3::decode(instruction_data)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    match request.action() {
        FractionalRetirementActionV3::Begin => {
            process_begin(program_id, accounts, &request, instruction_data)
        }
        FractionalRetirementActionV3::RetireCoordinate => {
            process_coordinate(program_id, accounts, &request, instruction_data)
        }
        FractionalRetirementActionV3::Finish => {
            process_finish(program_id, accounts, &request, instruction_data)
        }
    }
}

/// Execute one exact next-coordinate retirement.
#[inline(never)]
fn process_coordinate(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    authenticate_frame(program_id, accounts)?;
    let prepared = prepare_retirement(program_id, accounts, request, instruction_data)?;
    execute_retirement(program_id, accounts, request, &prepared)
}

/// The accounts every ordered-retirement act authenticates, whatever its frame.
///
/// Three frames of three different widths ask the same questions of the same
/// nine accounts. Naming the set once is what keeps begin, the coordinate walk
/// and finish from drifting into three subtly different ideas of what an
/// authenticated Fractional retirement is.
#[derive(Clone, Copy)]
struct SharedFrameV3<'accounts, 'info> {
    market: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    root: &'accounts AccountInfo<'info>,
    terms_raw: &'accounts AccountInfo<'info>,
    terms_staging: &'accounts AccountInfo<'info>,
    token_behavior_raw: &'accounts AccountInfo<'info>,
    token_behavior_staging: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> SharedFrameV3<'accounts, 'info> {
    fn at(
        accounts: &'accounts [AccountInfo<'info>],
        indices: [usize; 9],
    ) -> Result<Self, ProgramError> {
        let [
            market,
            registry,
            rent,
            trading_program,
            root,
            terms_raw,
            terms_staging,
            token_behavior_raw,
            token_behavior_staging,
        ] = indices;
        Ok(Self {
            market: account(accounts, market)?,
            registry: account(accounts, registry)?,
            rent: account(accounts, rent)?,
            trading_program: account(accounts, trading_program)?,
            root: account(accounts, root)?,
            terms_raw: account(accounts, terms_raw)?,
            terms_staging: account(accounts, terms_staging)?,
            token_behavior_raw: account(accounts, token_behavior_raw)?,
            token_behavior_staging: account(accounts, token_behavior_staging)?,
        })
    }

    fn coordinate(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        Self::at(
            accounts,
            [
                MARKET,
                REGISTRY,
                RENT,
                TRADING_PROGRAM,
                ROOT,
                TERMS_RAW,
                TERMS_STAGING,
                TOKEN_BEHAVIOR_RAW,
                TOKEN_BEHAVIOR_STAGING,
            ],
        )
    }

    fn begin(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        Self::at(
            accounts,
            [
                BEGIN_MARKET,
                BEGIN_REGISTRY,
                BEGIN_RENT,
                BEGIN_TRADING_PROGRAM,
                BEGIN_ROOT,
                BEGIN_TERMS_RAW,
                BEGIN_TERMS_STAGING,
                BEGIN_TOKEN_BEHAVIOR_RAW,
                BEGIN_TOKEN_BEHAVIOR_STAGING,
            ],
        )
    }

    fn finish(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        Self::at(
            accounts,
            [
                FINISH_MARKET,
                FINISH_REGISTRY,
                FINISH_RENT,
                FINISH_TRADING_PROGRAM,
                FINISH_ROOT,
                FINISH_TERMS_RAW,
                FINISH_TERMS_STAGING,
                FINISH_TOKEN_BEHAVIOR_RAW,
                FINISH_TOKEN_BEHAVIOR_STAGING,
            ],
        )
    }
}

/// Which revision the frozen producer root must be found at.
///
/// The root is written once and never mutated, so exactly one act may compare
/// it to the revision a caller supplied: the one that consumes it. Every later
/// act compares it to the anchor the cursor derives, because by then the
/// caller's revision belongs to the cursor and has already moved past the
/// root's. Naming the two cases as one type is what stops a later route from
/// picking whichever comparison happened to be written next to it -- the
/// mistake that made the coordinate walk unable to take a second step.
#[derive(Clone, Copy)]
enum RootRevisionBindingV3 {
    /// Begin: the request names the root's own frozen revision.
    Request,
    /// Every act after begin: the cursor's derived begin anchor.
    CursorAnchor(u64),
}

struct StaticRetirementFactsV3 {
    expected_mint: [u8; 32],
    market_generation: u64,
    market_revision: u64,
}

/// Facts an authenticated aggregate states about its Market.
#[derive(Clone, Copy)]
struct MarketFactsV3 {
    generation: u64,
    revision: u64,
    release_set: [u8; 32],
    realm_id: [u8; 32],
}

struct PreparedRetirementV3 {
    request_digest: [u8; 32],
    close_request: Vec<u8>,
    pre_cursor: Vec<u8>,
    post_cursor: Vec<u8>,
    pre_cursor_digest: [u8; 32],
    post_cursor_digest: [u8; 32],
    expected_mint: [u8; 32],
    post_revision: u64,
}

#[inline(never)]
fn prepare_retirement(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    instruction_data: &[u8],
) -> Result<Box<PreparedRetirementV3>, ProgramError> {
    let shared = SharedFrameV3::coordinate(accounts)?;
    let market = authenticate_terms_and_market(program_id, shared, request)?;
    let facts = authenticate_shard_mint(accounts, request, market)?;
    authenticate_behavior(shared, market, request)?;
    // The cursor is read before the root, because the cursor is what says
    // which root revision this walk is entitled to find.
    let anchor = authenticated_cursor(program_id, shared, account(accounts, CURSOR)?)?
        .root_revision_anchor()
        .map_err(|_| ClaimsSbfError::Representation)?;
    authenticate_root(shared, request, RootRevisionBindingV3::CursorAnchor(anchor))?;
    let (pre_cursor, post_cursor, pre_cursor_digest, post_cursor_digest, post_revision) =
        prepare_cursor(program_id, accounts, request, facts.expected_mint)?;
    let close_request = prepare_close_request(accounts, request, &facts)?;
    Ok(Box::new(PreparedRetirementV3 {
        request_digest: digest(instruction_data),
        close_request,
        pre_cursor,
        post_cursor,
        pre_cursor_digest,
        post_cursor_digest,
        expected_mint: facts.expected_mint,
        post_revision,
    }))
}

/// Authenticate the finalized terms record and the Claims aggregate.
///
/// The aggregate's own address is re-derived here rather than inferred from
/// the accounts that happen to point at it. Every act needs the Market's
/// generation and revision, and an act that read them from a substituted
/// account would be quoting a Market nobody selected.
#[inline(never)]
fn authenticate_terms_and_market(
    program_id: &Pubkey,
    shared: SharedFrameV3<'_, '_>,
    request: &FractionalRetirementRequestV3,
) -> Result<MarketFactsV3, ProgramError> {
    let input = request.input();
    let terms_data = shared
        .terms_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_rational_record(
        shared.registry.key,
        shared.terms_raw,
        shared.terms_staging,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        input.terms,
        &terms_data,
    )?;
    let terms = FractionalExposureTermsV2::decode(
        &terms_data,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: input.terms,
            finalized_terms_id: input.terms,
            recomputed_terms_digest: input.terms,
            finalized_terms_digest: input.terms,
            record_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    request
        .bind_terms(terms)
        .map_err(|_| ClaimsSbfError::Representation)?;
    drop(terms_data);

    let expected_aggregate = Pubkey::find_program_address(
        &LiabilityBasisMarketSeedsV2::new(input.market)
            .map_err(|_| ClaimsSbfError::Identity)?
            .as_slices(),
        program_id,
    )
    .0;
    let market_data = shared
        .market
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market =
        LiabilityBasisMarketViewV2::decode(&market_data).map_err(|_| ClaimsSbfError::Identity)?;
    if shared.market.key != &expected_aggregate
        || shared.market.owner != program_id
        || market.logical_market != input.market
        || market.release_set != input.release_set
        || market.registry_program != shared.registry.key.to_bytes()
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(MarketFactsV3 {
        generation: market.generation,
        revision: market.revision,
        release_set: market.release_set,
        realm_id: market.realm_id,
    })
}

/// Authenticate a live cursor account and return the state it carries.
///
/// The lamport check is a FLOOR, and the difference is whether a fractional
/// market can retire at all. A cursor is a keyless, off-curve address, so
/// anyone on the network may send it lamports at any time. Under the equality
/// this used to carry, one stranger lamport permanently froze the walk: the
/// cursor could never advance, never finish, and every shard holder's
/// collateral stayed behind a Position nothing could close -- for the price of
/// one lamport, once. This is the same reasoning `protocol_position_v2`'s
/// admission vacancy already records, arrived at from the other end: there a
/// donation blocked admission, here it blocked the exit.
///
/// Underfunding still refuses, which was the whole safety content of the
/// equality. Rent exemption is checked against the principal the cursor
/// declares rather than the balance it happens to hold, so a donation cannot
/// paper over a cursor that was never exempt.
#[inline(never)]
fn authenticated_cursor(
    program_id: &Pubkey,
    shared: SharedFrameV3<'_, '_>,
    cursor_account: &AccountInfo<'_>,
) -> Result<FractionalRetirementCursorV3, ProgramError> {
    let cursor_data = cursor_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let cursor = FractionalRetirementCursorV3::decode(&cursor_data)
        .map_err(|_| ClaimsSbfError::Representation)?;
    drop(cursor_data);
    let bump = [cursor.bump()];
    let expected_cursor = Pubkey::create_program_address(
        &[
            FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3,
            shared.root.key.as_ref(),
            &bump,
        ],
        program_id,
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    if cursor_account.key != &expected_cursor
        || cursor_account.owner != program_id
        || cursor_account.data_len() != FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3
        || cursor_account.lamports() < cursor.historical_rent_principal()
        || !funded_rent_persists_v1(cursor.historical_rent_principal())
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    Ok(cursor)
}

/// The coordinate walk's own extra admission: the exact selected shard Mint.
#[inline(never)]
fn authenticate_shard_mint(
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    market: MarketFactsV3,
) -> Result<StaticRetirementFactsV3, ProgramError> {
    let input = request.input();
    let shared = SharedFrameV3::coordinate(accounts)?;
    let terms_data = shared
        .terms_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let terms = FractionalExposureTermsV2::decode(
        &terms_data,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: input.terms,
            finalized_terms_id: input.terms,
            recomputed_terms_digest: input.terms,
            finalized_terms_digest: input.terms,
            record_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;

    let mint_account = account(accounts, SHARD_MINT)?;
    let token_program = account(accounts, TOKEN_PROGRAM)?;
    let expected_mint = terms
        .shard_mint(input.representation_coordinate)
        .map_err(|_| ClaimsSbfError::Token)?;
    if mint_account.key.to_bytes() != expected_mint
        || mint_account.owner != token_program.key
        || token_program.key.to_bytes() != input.token_program
    {
        return Err(ClaimsSbfError::Token.into());
    }
    let mint_data = mint_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let mint_facts = Token2022BehaviorProfileV2::check_mint(
        input.token_program,
        expected_mint,
        &mint_data,
        input.root,
        0,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    if mint_facts.base_supply() != 0 {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(StaticRetirementFactsV3 {
        expected_mint,
        market_generation: market.generation,
        market_revision: market.revision,
    })
}

#[inline(never)]
fn authenticate_behavior(
    shared: SharedFrameV3<'_, '_>,
    market: MarketFactsV3,
    request: &FractionalRetirementRequestV3,
) -> Result<(), ProgramError> {
    let input = request.input();
    let behavior_data = shared
        .token_behavior_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_finalized_rational_record(
        shared.registry.key,
        shared.token_behavior_raw,
        shared.token_behavior_staging,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        input.token_behavior,
        &behavior_data,
    )?;
    let behavior = TokenBehaviorSelectionV2::decode_for_authenticated_selection(
        &behavior_data,
        market.realm_id,
        input.release_set,
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    if behavior.token_program() != input.token_program || market.release_set != input.release_set {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_root(
    shared: SharedFrameV3<'_, '_>,
    request: &FractionalRetirementRequestV3,
    binding: RootRevisionBindingV3,
) -> Result<(), ProgramError> {
    let input = request.input();
    let trading_program = shared.trading_program;
    let root_account = shared.root;
    let root_data = root_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let composite_root =
        decode_fractional_capability_root_v4(&root_data).ok_or(ClaimsSbfError::Representation)?;
    let header = composite_root.header();
    let root = composite_root.state();
    let (expected_root, expected_root_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), trading_program.key);
    // The config split, pinned on its own code. The terms account was already
    // authenticated against `input.terms` by `authenticate_terms_market_and_mint`
    // before this function runs; re-decoding it here is the same pattern
    // `prepare_cursor` already uses, and keeps the selection check beside the
    // root authentication it belongs to.
    let terms_data = shared
        .terms_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let terms = FractionalExposureTermsV2::decode(
        &terms_data,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: input.terms,
            finalized_terms_id: input.terms,
            recomputed_terms_digest: input.terms,
            finalized_terms_digest: input.terms,
            record_authenticated: true,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    let mut selection_config = [0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
    encode_fractional_selection_config_v1(
        fractional_selection_config_from_terms_v1(terms),
        &mut selection_config,
    )
    .map_err(|_| ClaimsSbfError::SelectionConfig)?;
    let selected_config = digest(&selection_config);
    if header.selection().config().to_bytes() != selected_config {
        return Err(ClaimsSbfError::SelectionConfig.into());
    }
    let state_binding_matches = match (root.terms_v1(), root.selection_config_v2()) {
        (Some(historical_terms), None) => historical_terms == input.terms,
        (None, Some(current_config)) => current_config == selected_config,
        _ => false,
    };
    drop(terms_data);
    if root_account.key != &expected_root
        || root_account.key.to_bytes() != input.root
        || root_account.owner != trading_program.key
        || header.release_set().to_bytes() != input.release_set
        || header.market() != input.market
        || !state_binding_matches
        || root.bump() != expected_root_bump
        || root.market() != input.market
        || root.rent_beneficiary() != input.rent_credit
        // Never `input.expected_revision` on a post-begin act. The root is
        // frozen and the request's revision belongs to the cursor by then, so
        // that comparison is satisfiable for exactly one coordinate and
        // refuses every step after it.
        || root.revision()
            != match binding {
                RootRevisionBindingV3::Request => input.expected_revision,
                RootRevisionBindingV3::CursorAnchor(anchor) => anchor,
            }
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
#[inline(never)]
fn prepare_cursor(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    expected_mint: [u8; 32],
) -> Result<(Vec<u8>, Vec<u8>, [u8; 32], [u8; 32], u64), ProgramError> {
    let input = request.input();
    let terms_data = account(accounts, TERMS_RAW)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let terms = FractionalExposureTermsV2::decode(&terms_data, terms_admission(input.terms))
        .map_err(|_| ClaimsSbfError::Representation)?;
    let cursor_account = account(accounts, CURSOR)?;
    let cursor = authenticated_cursor(
        program_id,
        SharedFrameV3::coordinate(accounts)?,
        cursor_account,
    )?;
    let cursor_data = cursor_account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let pre_cursor = cursor_data.to_vec();
    let pre_cursor_digest = digest(&pre_cursor);
    drop(cursor_data);

    let position_data = account(accounts, POSITION)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position = LiabilityBasisPositionViewV2::decode(&position_data)
        .map_err(|_| ClaimsSbfError::Identity)?;
    let reserve_claims = position
        .balance(&position_data, input.representation_coordinate)
        .map_err(|_| ClaimsSbfError::Identity)?;
    drop(position_data);
    let cursor_candidate = cursor
        .advance(
            terms,
            *request,
            FractionalRetireCoordinateObservationV3 {
                shard_mint: expected_mint,
                shard_supply: 0,
                reserve_claims,
                mint_authenticated: true,
                reserve_authenticated: position.owner == input.root
                    && position.market_account == account(accounts, MARKET)?.key.to_bytes(),
            },
        )
        .map_err(|_| ClaimsSbfError::Representation)?;
    let post_cursor = cursor_candidate
        .to_bytes()
        .map_err(|_| ClaimsSbfError::Representation)?
        .to_vec();
    let post_cursor_digest = digest(&post_cursor);
    Ok((
        pre_cursor,
        post_cursor,
        pre_cursor_digest,
        post_cursor_digest,
        cursor_candidate.revision(),
    ))
}

#[inline(never)]
fn prepare_close_request(
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    facts: &StaticRetirementFactsV3,
) -> Result<Vec<u8>, ProgramError> {
    let input = request.input();
    let position_data = account(accounts, POSITION)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position = LiabilityBasisPositionViewV2::decode(&position_data)
        .map_err(|_| ClaimsSbfError::Identity)?;
    drop(position_data);
    let admission_data = account(accounts, ADMISSION)?
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let admission = ProtocolPositionAdmissionV2::decode(&admission_data)
        .map_err(|_| ClaimsSbfError::Identity)?;
    drop(admission_data);
    let close_request = ProtocolPositionRequestV2 {
        action: ProtocolPositionActionV2::Close,
        owner_kind: admission.owner_kind(),
        presence: ProtocolPositionPresenceV2::Existing,
        release_set: input.release_set,
        market: input.market,
        position_owner: input.root,
        parent_request_digest: admission.parent_request_digest(),
        rent_credit: input.rent_credit,
        rent_program: admission.rent_program(),
        generation: facts.market_generation,
        expected_market_revision: facts.market_revision,
        expected_position_revision: position.revision,
        observed_position_lamports: account(accounts, POSITION)?.lamports(),
        observed_admission_lamports: account(accounts, ADMISSION)?.lamports(),
        position_rent_principal: admission.position_rent_principal(),
        admission_rent_principal: admission.admission_rent_principal(),
        capability_descriptor: admission.capability_descriptor(),
        capability_outcome: admission.capability_outcome(),
    }
    .new()
    .map_err(|_| ClaimsSbfError::Identity)?;
    if admission.owner_kind() != ProtocolPositionOwnerKindV2::TradingRecord
        || admission.position_owner() != input.root
        || admission.release_set() != input.release_set
        || admission.market() != input.market
        || admission.rent_credit() != input.rent_credit
        || admission.generation() != facts.market_generation
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(close_request
        .to_bytes()
        .map_err(|_| ClaimsSbfError::Identity)?
        .to_vec())
}

#[inline(never)]
fn execute_retirement(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    prepared: &PreparedRetirementV3,
) -> Result<(), ProgramError> {
    let close_receipt_digest = execute_position_close(program_id, accounts, request, prepared)?;
    execute_mint_close(accounts)?;
    commit_cursor_and_emit(accounts, request, prepared, close_receipt_digest)
}

#[inline(never)]
fn execute_position_close(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    prepared: &PreparedRetirementV3,
) -> Result<[u8; 32], ProgramError> {
    let input = request.input();
    let close_accounts = accounts
        .get(..PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2)
        .ok_or(ClaimsSbfError::Accounts)?;
    let close_receipt = execute_parent_authenticated_close(
        program_id,
        close_accounts,
        &prepared.close_request,
        AuthenticatedProtocolPositionCloseParentV2 {
            release_set: input.release_set,
            market: input.market,
            parent_context: input.terms,
            parent_request_digest: prepared.request_digest,
            trading_root: input.root,
        },
    )?;
    Ok(digest(
        &close_receipt
            .to_bytes()
            .map_err(|_| ClaimsSbfError::Receipt)?,
    ))
}

#[inline(never)]
fn execute_mint_close(accounts: &[AccountInfo<'_>]) -> Result<(), ProgramError> {
    let token_program = account(accounts, TOKEN_PROGRAM)?;
    let mint_account = account(accounts, SHARD_MINT)?;
    let root_account = account(accounts, ROOT)?;
    let mut close_mint = token_instruction::close_account(
        token_program.key,
        mint_account.key,
        account(accounts, RENT_CREDIT)?.key,
        root_account.key,
        &[],
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    // The preceding native close moved lamports from Position and admission
    // into RentCredit. Include the complete conservation set as trailing CPI
    // metas while the runtime synchronizes account state before Token-2022.
    // Both remain Claims-owned; Token receives no authority to mutate them.
    close_mint.accounts.extend([
        solana_program::instruction::AccountMeta::new(*account(accounts, POSITION)?.key, false),
        solana_program::instruction::AccountMeta::new(*account(accounts, ADMISSION)?.key, false),
    ]);
    invoke(
        &close_mint,
        &[
            mint_account.clone(),
            account(accounts, RENT_CREDIT)?.clone(),
            root_account.clone(),
            token_program.clone(),
            account(accounts, POSITION)?.clone(),
            account(accounts, ADMISSION)?.clone(),
        ],
    )
    .map_err(|_| ClaimsSbfError::Token)?;
    if mint_account.owner != &system_program::ID
        || !mint_account.data_is_empty()
        || mint_account.lamports() != 0
    {
        return Err(ClaimsSbfError::Token.into());
    }
    Ok(())
}

#[inline(never)]
fn commit_cursor_and_emit(
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    prepared: &PreparedRetirementV3,
    close_receipt_digest: [u8; 32],
) -> Result<(), ProgramError> {
    let cursor_account = account(accounts, CURSOR)?;
    {
        let mut observed = cursor_account
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        if observed.as_ref() != prepared.pre_cursor.as_slice()
            || observed.len() != prepared.post_cursor.len()
        {
            return Err(ClaimsSbfError::Receipt.into());
        }
        observed.copy_from_slice(&prepared.post_cursor);
    }
    let receipt = FractionalRetirementCoordinateReceiptV3::new(
        *request,
        prepared.request_digest,
        close_receipt_digest,
        prepared.pre_cursor_digest,
        prepared.post_cursor_digest,
        prepared.expected_mint,
        prepared.post_revision,
    )
    .map_err(|_| ClaimsSbfError::Receipt)?;
    let receipt_bytes = receipt.to_bytes();
    if receipt_bytes.len() != FRACTIONAL_RETIREMENT_COORDINATE_RECEIPT_BYTES_V3 {
        return Err(ClaimsSbfError::Receipt.into());
    }
    set_return_data(&receipt_bytes);
    Ok(())
}

/// Create the cursor the ordered walk advances, once, on a terminal Market.
///
/// Anti-replay is the cursor account's own existence, so it has no code: a
/// second begin finds a Claims-owned, non-empty address and refuses at the
/// vacancy check. There is no counter and no cursor-about-the-cursor.
#[inline(never)]
fn process_begin(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    authenticate_begin_frame(program_id, accounts)?;
    let shared = SharedFrameV3::begin(accounts)?;
    let market = authenticate_terms_and_market(program_id, shared, request)?;
    authenticate_behavior(shared, market, request)?;
    // Begin is the one act that consumes the root's own frozen revision.
    authenticate_root(shared, request, RootRevisionBindingV3::Request)?;
    authenticate_terminal_market(accounts, request, market)?;

    let cursor_account = account(accounts, BEGIN_CURSOR)?;
    let rent = Rent::from_account_info(shared.rent).map_err(|_| ClaimsSbfError::Accounts)?;
    let principal = rent.minimum_balance(FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3);
    let bump = authenticate_cursor_vacancy(program_id, shared, cursor_account, principal)?;

    let terms_data = shared
        .terms_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let terms =
        FractionalExposureTermsV2::decode(&terms_data, terms_admission(request.input().terms))
            .map_err(|_| ClaimsSbfError::Representation)?;
    let cursor = FractionalRetirementCursorV3::begin(
        terms,
        *request,
        FractionalRetirementCursorInputV3 {
            bump,
            pre_revision: request.input().expected_revision,
            historical_rent_principal: principal,
        },
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    drop(terms_data);
    let candidate = cursor
        .to_bytes()
        .map_err(|_| ClaimsSbfError::Representation)?;

    // Serialized before the first mutation, so a receipt-layout error can
    // never strand an allocated cursor without evidence.
    let receipt = lifecycle_receipt(
        cursor,
        request,
        instruction_data,
        FractionalRetirementLifecycleObservationV3 {
            cursor: cursor_account.key.to_bytes(),
            cursor_digest: digest(&candidate),
            cursor_rent_principal: principal,
            post_revision: cursor.revision(),
            lamports_settled: 0,
        },
    )?;

    fund_and_allocate_cursor(
        program_id,
        account(accounts, BEGIN_PAYER)?,
        cursor_account,
        account(accounts, BEGIN_SYSTEM)?,
        shared.root,
        bump,
        principal,
    )?;
    {
        let mut written = cursor_account
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        if written.len() != candidate.len() || written.iter().any(|byte| *byte != 0) {
            return Err(ClaimsSbfError::Receipt.into());
        }
        written.copy_from_slice(&candidate);
    }
    set_return_data(&receipt);
    Ok(())
}

/// Close the completed cursor and settle every lamport it holds.
///
/// Anti-replay is again the account's own existence: finish leaves a vacant,
/// system-owned address behind, and a second finish cannot decode a cursor
/// that is no longer there.
#[inline(never)]
fn process_finish(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    authenticate_finish_frame(program_id, accounts)?;
    let shared = SharedFrameV3::finish(accounts)?;
    let market = authenticate_terms_and_market(program_id, shared, request)?;
    authenticate_behavior(shared, market, request)?;

    let cursor_account = account(accounts, FINISH_CURSOR)?;
    let cursor = authenticated_cursor(program_id, shared, cursor_account)?;
    authenticate_root(
        shared,
        request,
        RootRevisionBindingV3::CursorAnchor(
            cursor
                .root_revision_anchor()
                .map_err(|_| ClaimsSbfError::Representation)?,
        ),
    )?;

    let terms_data = shared
        .terms_raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let terms =
        FractionalExposureTermsV2::decode(&terms_data, terms_admission(request.input().terms))
            .map_err(|_| ClaimsSbfError::Representation)?;
    // The completeness gate. `finish` refuses unless every coordinate the
    // terms declare has already advanced, so a cursor abandoned mid-walk
    // cannot be closed out from under the coordinates it still owes.
    let finish = cursor
        .finish(terms, *request)
        .map_err(|_| ClaimsSbfError::Representation)?;
    drop(terms_data);

    let rent_credit = account(accounts, FINISH_RENT_CREDIT)?;
    let rent_program = account(accounts, FINISH_RENT_PROGRAM)?;
    authenticate_rent_credit(
        rent_credit,
        rent_program,
        LifecycleRentCreditIdentityV2 {
            // From the CURSOR, which took it from the root's rent beneficiary
            // at begin. The caller does not get to name where the rent goes.
            rent_credit: finish.rent_credit,
            // Not independently pinned, and it does not need to be. The
            // Position close reads its rent program out of a persisted
            // admission; a cursor records no such field, so this argument is
            // the supplied account naming itself. What makes that safe is the
            // line above: the ADDRESS is fixed, and `authenticate_rent_credit`
            // then requires that exact account to be owned by the supplied
            // program AND to re-derive to itself from its own persisted seeds
            // under that program's key. A substituted program would have to
            // already own the one address the root chose.
            rent_program: rent_program.key.to_bytes(),
            market: request.input().market,
            release_set: request.input().release_set,
            generation: market.generation,
        },
    )?;

    // Everything the account holds, not just the principal it declared: the
    // cursor is about to stop existing, and a lamport left in it is a lamport
    // burned. `rent_after` is computed before the first mutation.
    let settled = cursor_account.lamports();
    let rent_before = rent_credit.lamports();
    let rent_after = rent_before
        .checked_add(settled)
        .ok_or(ClaimsSbfError::Receipt)?;

    let cursor_digest = {
        let data = cursor_account
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        digest(&data)
    };
    let receipt = lifecycle_receipt(
        cursor,
        request,
        instruction_data,
        FractionalRetirementLifecycleObservationV3 {
            cursor: cursor_account.key.to_bytes(),
            cursor_digest,
            cursor_rent_principal: finish.cursor_rent_principal,
            post_revision: finish.terminal_revision,
            lamports_settled: settled,
        },
    )?;

    close_cursor(cursor_account, rent_credit, rent_after)?;
    set_return_data(&receipt);
    Ok(())
}

fn terms_admission(terms: [u8; 32]) -> FractionalExposureTermsAdmissionV2 {
    FractionalExposureTermsAdmissionV2 {
        selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        selected_terms_id: terms,
        finalized_terms_id: terms,
        recomputed_terms_digest: terms,
        finalized_terms_digest: terms,
        record_authenticated: true,
    }
}

#[inline(never)]
fn lifecycle_receipt(
    cursor: FractionalRetirementCursorV3,
    request: &FractionalRetirementRequestV3,
    instruction_data: &[u8],
    observed: FractionalRetirementLifecycleObservationV3,
) -> Result<[u8; FRACTIONAL_RETIREMENT_LIFECYCLE_RECEIPT_BYTES_V3], ProgramError> {
    let request_digest = digest(instruction_data);
    let receipt =
        FractionalRetirementLifecycleReceiptV3::new(cursor, *request, request_digest, observed)
            .map_err(|_| ClaimsSbfError::Receipt)?;
    receipt
        .verify_for(*request, request_digest)
        .map_err(|_| ClaimsSbfError::Receipt)?;
    Ok(receipt.to_bytes())
}

/// The Market must have resolved before its Fractional root may retire.
///
/// `TerminalOrRetiring` and not `Exactly(Terminal)`: `begin_retiring` is
/// permissionless, so a gate that refused there would let any stranger push a
/// Market one phase forward and strand a retirement that had not started yet.
/// The terminal receipt is checked as well even though the phase implies it,
/// because a checked invariant is one an implementer cannot silently delete.
#[inline(never)]
fn authenticate_terminal_market(
    accounts: &[AccountInfo<'_>],
    request: &FractionalRetirementRequestV3,
    market: MarketFactsV3,
) -> Result<(), ProgramError> {
    let core_market = account(accounts, BEGIN_CORE_MARKET)?;
    let core_program = account(accounts, BEGIN_CORE_PROGRAM)?;
    let input = request.input();
    let core_data = core_market
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if core_market.owner != core_program.key
        || core_market.key.to_bytes() != input.market
        || core_data.len() != STATE_BYTES
        || !core_program.executable
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let core = CoreState::decode(&core_data).map_err(|_| ClaimsSbfError::Identity)?;
    let expected_core = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        core_program.key,
    )
    .0;
    if &expected_core != core_market.key
        || core.identity.market_id.to_bytes() != input.market
        || core.identity.selected_release_set.to_bytes() != input.release_set
        || core.identity.generation != market.generation
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    if !CLAIMS_SETTLED_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(core.phase)
        || core.terminal_receipt.is_none()
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

/// Refuse anything but a vacant, adequately funded cursor address.
#[inline(never)]
fn authenticate_cursor_vacancy(
    program_id: &Pubkey,
    shared: SharedFrameV3<'_, '_>,
    cursor_account: &AccountInfo<'_>,
    principal: u64,
) -> Result<u8, ProgramError> {
    let (expected, bump) = Pubkey::find_program_address(
        &[
            FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3,
            shared.root.key.as_ref(),
        ],
        program_id,
    );
    if cursor_account.key != &expected
        || cursor_account.owner != &system_program::ID
        || !cursor_account.data_is_empty()
        || principal == 0
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    Ok(bump)
}

/// Top the cursor up to rent exemption, then allocate and assign it.
///
/// A top-up rather than a `create_account`, and a top-up rather than a fixed
/// transfer: the address is keyless and anyone may have donated to it, so a
/// route that insisted on moving the whole principal would refuse on exactly
/// the addresses a griefer had already funded.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn fund_and_allocate_cursor<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    cursor_account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    root: &AccountInfo<'info>,
    bump: u8,
    principal: u64,
) -> Result<(), ProgramError> {
    let top_up = principal.saturating_sub(cursor_account.lamports());
    if top_up != 0 {
        let funding = transfer(payer.key, cursor_account.key, top_up);
        invoke(
            &Instruction {
                program_id: funding.program_id,
                accounts: funding.accounts,
                data: funding.data,
            },
            &[payer.clone(), cursor_account.clone(), system.clone()],
        )
        .map_err(|_| ClaimsSbfError::Representation)?;
    }
    let bump_seed = [bump];
    let seeds: &[&[u8]] = &[
        FRACTIONAL_RETIREMENT_CURSOR_PDA_SEED_V3,
        root.key.as_ref(),
        &bump_seed,
    ];
    let space = u64::try_from(FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3)
        .map_err(|_| ClaimsSbfError::Accounts)?;
    for instruction in [
        allocate(cursor_account.key, space),
        assign(cursor_account.key, program_id),
    ] {
        invoke_signed(
            &Instruction {
                program_id: instruction.program_id,
                accounts: instruction.accounts,
                data: instruction.data,
            },
            &[cursor_account.clone(), system.clone()],
            &[seeds],
        )
        .map_err(|_| ClaimsSbfError::Representation)?;
    }
    if cursor_account.owner != program_id
        || cursor_account.data_len() != FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3
        || cursor_account.lamports() < principal
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    Ok(())
}

/// Zero, drain, shrink, hand back to System, and re-verify all four.
#[inline(never)]
fn close_cursor(
    cursor_account: &AccountInfo<'_>,
    rent_credit: &AccountInfo<'_>,
    rent_after: u64,
) -> Result<(), ProgramError> {
    {
        let mut data = cursor_account
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        data.fill(0);
    }
    {
        let mut cursor_lamports = cursor_account
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        let mut credit_lamports = rent_credit
            .try_borrow_mut_lamports()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        **cursor_lamports = 0;
        **credit_lamports = rent_after;
    }
    cursor_account
        .resize(0)
        .map_err(|_| ClaimsSbfError::Accounts)?;
    cursor_account.assign(&system_program::ID);
    if cursor_account.lamports() != 0
        || !cursor_account.data_is_empty()
        || cursor_account.owner != &system_program::ID
        || rent_credit.lamports() != rent_after
    {
        return Err(ClaimsSbfError::Receipt.into());
    }
    Ok(())
}

#[inline(never)]
fn authenticate_begin_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    authenticate_lifecycle_frame(
        program_id,
        accounts,
        FRACTIONAL_RETIREMENT_BEGIN_ACCOUNT_COUNT_V3,
        BEGIN_CLAIMS_PROGRAM,
        &|index| match index {
            BEGIN_PAYER => Some(FrameRoleV3::SigningPayer),
            BEGIN_CURSOR => Some(FrameRoleV3::Written),
            BEGIN_CORE_PROGRAM
            | BEGIN_REGISTRY
            | BEGIN_TRADING_PROGRAM
            | BEGIN_CLAIMS_PROGRAM
            | BEGIN_SYSTEM => Some(FrameRoleV3::Program),
            BEGIN_MARKET
            | BEGIN_CORE_MARKET
            | BEGIN_RENT
            | BEGIN_ROOT
            // Read, and this is the coordinate the exemption exists for:
            // `Finish` must take the RentCredit writable, and a readonly pin
            // here would make the two acts unbatchable.
            | BEGIN_RENT_CREDIT
            | BEGIN_TERMS_RAW
            | BEGIN_TERMS_STAGING
            | BEGIN_TOKEN_BEHAVIOR_RAW
            | BEGIN_TOKEN_BEHAVIOR_STAGING => Some(FrameRoleV3::Read),
            _ => None,
        },
    )
}

#[inline(never)]
fn authenticate_finish_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    authenticate_lifecycle_frame(
        program_id,
        accounts,
        FRACTIONAL_RETIREMENT_FINISH_ACCOUNT_COUNT_V3,
        FINISH_CLAIMS_PROGRAM,
        &|index| match index {
            FINISH_CURSOR | FINISH_RENT_CREDIT => Some(FrameRoleV3::Written),
            FINISH_REGISTRY
            | FINISH_TRADING_PROGRAM
            | FINISH_CLAIMS_PROGRAM
            | FINISH_RENT_PROGRAM => Some(FrameRoleV3::Program),
            FINISH_MARKET
            | FINISH_RENT
            | FINISH_ROOT
            | FINISH_TERMS_RAW
            | FINISH_TERMS_STAGING
            | FINISH_TOKEN_BEHAVIOR_RAW
            | FINISH_TOKEN_BEHAVIOR_STAGING => Some(FrameRoleV3::Read),
            _ => None,
        },
    )
}

/// What one lifecycle-frame coordinate must be.
///
/// Writability is pinned in ONE DIRECTION, and the asymmetry is the whole
/// point. `is_writable` is a TRANSACTION-level property: the runtime merges an
/// account's privileges across every instruction of the transaction that names
/// it, so a coordinate this route only reads still arrives `true` whenever the
/// caller's OTHER instruction had to write it. Demanding writable is a
/// statement about this instruction and is enforceable. Demanding READONLY is a
/// statement about the caller's whole transaction, and it is not this route's
/// to make -- it forbids compositions rather than protecting anything here.
///
/// Not hypothetical for this family, and the exemption is one coordinate wide
/// for a named reason: `Finish` must take the RentCredit writable and `Begin`
/// only reads it, so a readonly pin at `BEGIN_RENT_CREDIT` makes the two acts
/// of one walk unbatchable. That is the shape `16351a13` fixed on Custody,
/// where a checkpoint pinned readonly could never be composed with the
/// Trading ingest that is its documented atomic partner.
///
/// Nothing is lost by it. A read-only coordinate arriving writable changes no
/// byte this route reads, and every account it could reach through a CPI is
/// `Written` or `SigningPayer` here.
#[derive(Clone, Copy)]
enum FrameRoleV3 {
    /// Funds the cursor: both the signature and the write are this
    /// instruction's own requirements.
    SigningPayer,
    /// This route writes it.
    Written,
    /// An executable this route only names.
    Program,
    /// This route only reads it.
    Read,
}

impl FrameRoleV3 {
    fn admits(self, observed: &AccountInfo<'_>) -> bool {
        let (signer, executable) = match self {
            Self::SigningPayer => (true, false),
            Self::Written | Self::Read => (false, false),
            Self::Program => (false, true),
        };
        let writability_is_free = matches!(self, Self::Program | Self::Read);
        observed.is_signer == signer
            && observed.executable == executable
            && (writability_is_free || observed.is_writable)
    }
}

/// Named-role privileges over a fixed table, plus a distinct address set.
///
/// `None` from the table means the index has no name in this frame, and an
/// unnamed coordinate refuses rather than inheriting whichever arm it was
/// written beside.
#[inline(never)]
fn authenticate_lifecycle_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    count: usize,
    claims_program: usize,
    roles: &dyn Fn(usize) -> Option<FrameRoleV3>,
) -> Result<(), ProgramError> {
    if accounts.len() != count || account(accounts, claims_program)?.key != program_id {
        return Err(ClaimsSbfError::Accounts.into());
    }
    for (index, observed) in accounts.iter().enumerate() {
        if !roles(index).is_some_and(|role| role.admits(observed)) {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    // Every account in these two frames is a distinct role, so unlike the
    // coordinate frame there is no legitimately aliasable pair to exclude.
    for (offset, left) in accounts.iter().enumerate() {
        if accounts
            .get(offset.saturating_add(1)..)
            .is_none_or(|tail| tail.iter().any(|right| right.key == left.key))
        {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    Ok(())
}

fn authenticate_frame(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    if accounts.len() != FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3
        || account(accounts, CLAIMS_PROGRAM)?.key != program_id
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    for (index, observed) in accounts
        .iter()
        .enumerate()
        .skip(PROTOCOL_POSITION_CLOSE_ACCOUNT_COUNT_V2)
    {
        let (signer, writable, executable) = match index {
            CURSOR | SHARD_MINT => (false, true, false),
            TOKEN_PROGRAM => (false, false, true),
            TERMS_RAW | TERMS_STAGING | TOKEN_BEHAVIOR_RAW | TOKEN_BEHAVIOR_STAGING => {
                (false, false, false)
            }
            _ => return Err(ClaimsSbfError::Accounts.into()),
        };
        if observed.is_signer != signer
            || observed.is_writable != writable
            || observed.executable != executable
        {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    let distinct = [
        AUTHORITY,
        MARKET,
        POSITION,
        ADMISSION,
        REGISTRY,
        TRADING_PROGRAM,
        CLAIMS_PROGRAM,
        ROOT,
        RENT_CREDIT,
        RENT_PROGRAM,
        CURSOR,
        TERMS_RAW,
        TERMS_STAGING,
        TOKEN_BEHAVIOR_RAW,
        TOKEN_BEHAVIOR_STAGING,
        SHARD_MINT,
        TOKEN_PROGRAM,
    ];
    for (offset, left) in distinct.iter().copied().enumerate() {
        if distinct.get(offset.saturating_add(1)..).is_none_or(|tail| {
            tail.iter().any(|right| {
                accounts
                    .get(*right)
                    .zip(accounts.get(left))
                    .is_some_and(|(right, left)| right.key == left.key)
            })
        }) {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| ClaimsSbfError::Accounts.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinate_frame_is_exact_and_below_both_lock_boundaries() {
        assert_eq!(FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3, 22);
        assert!(FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3 <= 64);
        assert!(FRACTIONAL_RETIREMENT_COORDINATE_ACCOUNT_COUNT_V3 < 65);
        assert_eq!(FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3, 288);
        assert!(FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3 <= 1_232);
    }
}
