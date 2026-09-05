//! Split and merge as USER ACTS that move collateral.
//!
//! Decision 0029 item 5 ruled BUILD: *"so a holder can reshape a position
//! without a counterparty"*. This is the outer route
//! `dclutch-claims::conservation` was written for and named as absent
//! in its own words -- *"Nothing on chain dispatches
//! `CLAIMS_CONSERVATION_REQUEST_MAGIC_V1`; no operator builds it; no client can
//! send it. Split and merge remain UNIMPLEMENTED as user acts"*. The operator
//! half landed since (`crates/dclutch-operator/src/claims_conservation_v1.rs`);
//! this is the dispatcher, and with it `claims.conserve`/`DCLCNS01` stops being
//! the tree's one orphan magic and `CustodyRequired 0x5006` stops being a dead
//! refusal.
//!
//! # What this route is, against the one it does not replace
//!
//! The generic `ClaimsPlanV1` route can already ask the economic kernel for a
//! complete-set mint. What it cannot do is MOVE COLLATERAL: its mint credits
//! the aggregate's Hoard SCALAR and transfers no atoms, so claims come into
//! existence against a Hoard that received nothing. That route is
//! `ECONOMIC_SLICE_MIGRATION_ONLY` and stays so. This one is the user act: one
//! uniform signed complete-set delta and one Claims-role Custody transfer of
//! exactly `quantity * basis_scale` collateral atoms, derived from the SAME
//! request so that neither half can be constructed without the other.
//!
//! # The two shapes, and why the wire does not distinguish them
//!
//! A categorical Market's complete set lives in one Position. A refunding
//! Market's lives in two: the ordinary coordinates with the holder, the failure
//! coordinate with the Market's own escrow (decision 0025 item 2). The wire
//! says nothing about which -- the RECORD does, through
//! `categorical_refunds_on_failure_v3`, exactly as founding does -- and the
//! quantity vector is the same uniform vector either way, because
//! `MintRefundingCompleteSet` and `MergeRefundingCompleteSet` take the
//! categorical vector and seat the coordinates themselves. That is why the
//! conservation contract needed no change to carry the refunding shape.
//!
//! # The order, both ways
//!
//! A SPLIT moves the collateral first and mints second: claims must never
//! exist against a Hoard that has not yet received their backing, even inside
//! one instruction. A MERGE burns first and pays second, for the mirror
//! reason. Both are atomic, and the ordering is what makes an intermediate
//! state readable rather than merely unreachable.
//!
//! # Why a split needs the delegated Custody wire
//!
//! A split debits the actor's OWN external token account, and Custody's V1
//! `Transfer` refuses an `External` source outright, precisely so that an
//! apparently correct balance delta cannot leave hidden delegated spending
//! authority behind. The actor therefore approves exactly `collateral_atoms`
//! to the Custody transfer authority in the same transaction, and the single
//! transfer consumes all of it: `allowance_before == collateral_atoms`,
//! `allowance_after == 0`. Any residual allowance after a split would be the
//! hidden authority that wire exists to forbid.

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use dclutch_claims::conservation::{
    CLAIMS_CONSERVATION_REQUEST_BYTES_V1, ClaimsConservationDirectionV1,
    ClaimsConservationRequestV1,
};
use dclutch_claims::{
    liability_basis_state_v2::{
        LiabilityBasisMarketViewV2 as MarketViewV2, LiabilityBasisPositionViewV2 as PositionViewV2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyRequestV1, CustodyVaultSeedsV1,
};
use dclutch_product::economic_slice::{
    BasketAction, BasketFrame, execute_basket, market_hoard, market_supply, position_native,
    position_revision,
};
use dclutch_product::svm_reader::{FinalizedRecordFrameV2, ProductRuntimeFrameV3};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_source::MarketPrincipalCapSetsV1;
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};

use crate::affine_batch_v2::authenticate_runtime_product_basis_core_with_rent_v3;
use crate::claims_cu_checkpoint;
use crate::market_admission_v1::CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1;
use crate::{ClaimsSbfError, FailureEscrowIdentityV1};

/// Exact physical account count of the sole conservation frame.
///
/// The escrow Position rides on EVERY conservation act, categorical or not,
/// for the same reason it rides on every founding: the frame is fixed, so a
/// caller cannot signal a Market's shape by which accounts it supplies, and a
/// categorical Market can never be handed an escrow other than the one its own
/// address arithmetic gives.
pub const CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1: usize = 29;

const OWNER: usize = 0;
const AGGREGATE: usize = 1;
const POSITION: usize = 2;
const ESCROW_POSITION: usize = 3;
const CORE_MARKET: usize = 4;
const BASIS_RECORD: usize = 5;
const BASIS_STAGING: usize = 6;
const PRODUCT_RECORD: usize = 7;
const PRODUCT_STAGING: usize = 8;
const RESULT_RECORD: usize = 9;
const RESULT_STAGING: usize = 10;
const PORTFOLIO_RECORD: usize = 11;
const PORTFOLIO_STAGING: usize = 12;
const CACHE: usize = 13;
const REGISTRY: usize = 14;
const CLAIMS_PROGRAM: usize = 15;
const CLAIMS_PROGRAMDATA: usize = 16;
const CORE_PROGRAM: usize = 17;
const CORE_PROGRAMDATA: usize = 18;
const CUSTODY_CALLER_AUTHORITY: usize = 19;
const CUSTODY_PROGRAM: usize = 20;
const CUSTODY_REPLAY: usize = 21;
const HOARD_VAULT: usize = 22;
const EXTERNAL_COLLATERAL: usize = 23;
const COLLATERAL_MINT: usize = 24;
const TOKEN_PROGRAM: usize = 25;
const CUSTODY_AUTHORITY: usize = 26;
const REALM_RECORD: usize = 27;
const REALM_STAGING: usize = 28;

#[derive(Clone, Copy)]
struct ConservationAccounts<'accounts, 'info> {
    owner: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    escrow_position: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    basis_staging: &'accounts AccountInfo<'info>,
    product_record: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    result_record: &'accounts AccountInfo<'info>,
    result_staging: &'accounts AccountInfo<'info>,
    portfolio_record: &'accounts AccountInfo<'info>,
    portfolio_staging: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    custody_caller_authority: &'accounts AccountInfo<'info>,
    custody_program: &'accounts AccountInfo<'info>,
    custody_replay: &'accounts AccountInfo<'info>,
    hoard_vault: &'accounts AccountInfo<'info>,
    external_collateral: &'accounts AccountInfo<'info>,
    collateral_mint: &'accounts AccountInfo<'info>,
    token_program: &'accounts AccountInfo<'info>,
    custody_authority: &'accounts AccountInfo<'info>,
    realm_record: &'accounts AccountInfo<'info>,
    realm_staging: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> ConservationAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() != CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1 {
            return Err(ClaimsSbfError::Accounts.into());
        }
        let at = |index: usize| -> Result<&'accounts AccountInfo<'info>, ProgramError> {
            accounts.get(index).ok_or(ClaimsSbfError::Accounts.into())
        };
        Ok(Self {
            owner: at(OWNER)?,
            aggregate: at(AGGREGATE)?,
            position: at(POSITION)?,
            escrow_position: at(ESCROW_POSITION)?,
            core_market: at(CORE_MARKET)?,
            basis_record: at(BASIS_RECORD)?,
            basis_staging: at(BASIS_STAGING)?,
            product_record: at(PRODUCT_RECORD)?,
            product_staging: at(PRODUCT_STAGING)?,
            result_record: at(RESULT_RECORD)?,
            result_staging: at(RESULT_STAGING)?,
            portfolio_record: at(PORTFOLIO_RECORD)?,
            portfolio_staging: at(PORTFOLIO_STAGING)?,
            cache: at(CACHE)?,
            registry: at(REGISTRY)?,
            claims_program: at(CLAIMS_PROGRAM)?,
            claims_programdata: at(CLAIMS_PROGRAMDATA)?,
            core_program: at(CORE_PROGRAM)?,
            core_programdata: at(CORE_PROGRAMDATA)?,
            custody_caller_authority: at(CUSTODY_CALLER_AUTHORITY)?,
            custody_program: at(CUSTODY_PROGRAM)?,
            custody_replay: at(CUSTODY_REPLAY)?,
            hoard_vault: at(HOARD_VAULT)?,
            external_collateral: at(EXTERNAL_COLLATERAL)?,
            collateral_mint: at(COLLATERAL_MINT)?,
            token_program: at(TOKEN_PROGRAM)?,
            custody_authority: at(CUSTODY_AUTHORITY)?,
            realm_record: at(REALM_RECORD)?,
            realm_staging: at(REALM_STAGING)?,
        })
    }
}

/// Whether this instruction selects the conservation family.
pub fn is_claims_conservation_v1(instruction_data: &[u8]) -> bool {
    instruction_data.len() == CLAIMS_CONSERVATION_REQUEST_BYTES_V1
        && instruction_data
            .get(..dclutch_claims::conservation::CLAIMS_CONSERVATION_REQUEST_MAGIC_V1.len())
            == Some(
                dclutch_claims::conservation::CLAIMS_CONSERVATION_REQUEST_MAGIC_V1
                    .as_slice(),
            )
}

/// Execute one conservative complete-set act signed by its own Position owner.
#[inline(never)]
pub fn process(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    claims_cu_checkpoint!("conserve-enter");
    let request = ClaimsConservationRequestV1::decode(instruction_data)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    request
        .validate()
        .map_err(|_| ClaimsSbfError::Instruction)?;
    let accounts = ConservationAccounts::parse(account_infos)?;
    authenticate_privileges(program_id, accounts, request)?;
    claims_cu_checkpoint!("conserve-privileges");
    let market = authenticate_market_and_records(program_id, accounts, request)?;
    claims_cu_checkpoint!("conserve-records");
    let escrow = authenticate_positions_and_escrow(program_id, accounts, request, market)?;
    claims_cu_checkpoint!("conserve-positions");

    let split = matches!(request.direction, ClaimsConservationDirectionV1::Split);
    let action = conservation_basket_action_v1(request.direction, escrow.seated);

    // A SPLIT moves the collateral before the claims exist; a MERGE burns the
    // claims before the collateral leaves. Neither ordering is observable from
    // outside the instruction, and both are the readable one.
    if split {
        move_collateral(program_id, accounts, request)?;
        claims_cu_checkpoint!("conserve-collateral");
        apply_economics(accounts, request, action, escrow)?;
    } else {
        apply_economics(accounts, request, action, escrow)?;
        claims_cu_checkpoint!("conserve-economics");
        move_collateral(program_id, accounts, request)?;
    }
    claims_cu_checkpoint!("conserve-done");
    Ok(())
}

/// The kernel action one direction takes on one Market shape.
///
/// Four cells and no fifth. The refunding arms move the aggregate exactly as
/// their categorical namesakes do -- Hoard, supply and the native partition by
/// the same amount at every coordinate -- so the census reads a refunding
/// Market with no new compartment and every conservation already proved
/// governs it. The whole difference is which Position each coordinate lands
/// in, and the kernel owns that.
const fn conservation_basket_action_v1(
    direction: ClaimsConservationDirectionV1,
    seated: bool,
) -> BasketAction {
    match (direction, seated) {
        (ClaimsConservationDirectionV1::Split, false) => BasketAction::MintCompleteSet,
        (ClaimsConservationDirectionV1::Split, true) => BasketAction::MintRefundingCompleteSet,
        (ClaimsConservationDirectionV1::Merge, false) => BasketAction::MergeCompleteSet,
        (ClaimsConservationDirectionV1::Merge, true) => BasketAction::MergeRefundingCompleteSet,
    }
}

fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
) -> Result<(), ProgramError> {
    if !accounts.owner.is_signer
        || accounts.owner.is_writable
        || accounts.owner.executable
        || accounts.owner.key.to_bytes() != request.owner
        || !accounts.aggregate.is_writable
        || !accounts.position.is_writable
        || !accounts.custody_replay.is_writable
        || !accounts.hoard_vault.is_writable
        || !accounts.external_collateral.is_writable
        || accounts.claims_program.key != program_id
        || accounts.claims_program.key.to_bytes() != request.claims_program
        || !accounts.claims_program.executable
        || !accounts.core_program.executable
        || !accounts.custody_program.executable
        || !accounts.registry.executable
        || accounts.aggregate.key.to_bytes() != request.aggregate
        || accounts.position.key.to_bytes() != request.position
        || accounts.hoard_vault.key.to_bytes() != request.hoard_vault
        || accounts.external_collateral.key.to_bytes() != request.external_collateral
        || accounts.collateral_mint.key.to_bytes() != request.mint
        || accounts.token_program.key.to_bytes() != request.token_program
        || accounts.core_market.key.to_bytes() != request.market
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    for readonly in [
        accounts.core_market,
        accounts.basis_record,
        accounts.basis_staging,
        accounts.product_record,
        accounts.product_staging,
        accounts.result_record,
        accounts.result_staging,
        accounts.portfolio_record,
        accounts.portfolio_staging,
        accounts.cache,
        accounts.registry,
        accounts.claims_program,
        accounts.claims_programdata,
        accounts.core_program,
        accounts.core_programdata,
        accounts.custody_caller_authority,
        accounts.custody_program,
        accounts.collateral_mint,
        accounts.token_program,
        accounts.custody_authority,
        accounts.realm_record,
        accounts.realm_staging,
    ] {
        if readonly.is_signer || readonly.is_writable {
            return Err(ClaimsSbfError::Accounts.into());
        }
    }
    Ok(())
}

/// Join the Claims aggregate, the Product graph, the basis record and Core.
///
/// The actor signs for their own Position and for nothing else, so unlike the
/// routed complete-set act there is no caller authority whose seeds already
/// committed to a Product record: this route authenticates the whole graph
/// itself, as the user-signed Position lifecycle does.
#[inline(never)]
fn authenticate_market_and_records(
    program_id: &Pubkey,
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
) -> Result<AuthenticatedConservationMarketV1, ProgramError> {
    if accounts.aggregate.owner != program_id {
        return Err(ClaimsSbfError::Identity.into());
    }
    let aggregate = accounts
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market = MarketViewV2::decode(&aggregate).map_err(|_| ClaimsSbfError::Identity)?;
    if market.logical_market != request.market
        || market.release_set != request.release_set
        || market.registry_program != accounts.registry.key.to_bytes()
        || market.basis_id != request.semantic_basis_id
        || market.custody_context != request.custody_context
        || market.realm_id != request.realm
        || market.generation != request.generation
        || market.claim_count != request.claim_count
        || market.revision != request.expected_market_revision
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let outstanding_sets = market_hoard(&aggregate).map_err(|_| ClaimsSbfError::Economic)?;
    drop(aggregate);
    if hash(
        &accounts
            .product_record
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?,
    )
    .to_bytes()
        != request.product_record_digest
        || hash(
            &accounts
                .basis_record
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?,
        )
        .to_bytes()
            != request.linked_basis_record_digest
    {
        return Err(ClaimsSbfError::Representation.into());
    }
    let admitted = authenticate_runtime_product_basis_core_with_rent_v3(
        accounts.registry,
        accounts.core_market,
        accounts.core_program,
        ProductRuntimeFrameV3 {
            product: FinalizedRecordFrameV2 {
                raw: accounts.product_record,
                staging: accounts.product_staging,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: accounts.result_record,
                staging: accounts.result_staging,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: accounts.portfolio_record,
                staging: accounts.portfolio_staging,
            },
            linked_basis: FinalizedRecordFrameV2 {
                raw: accounts.basis_record,
                staging: accounts.basis_staging,
            },
        },
        market,
        request.product_record_digest,
        request.linked_basis_record_digest,
        CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1,
    )
    .map_err(|_| ClaimsSbfError::Representation)?;
    // Core's sole runtime principal cap, at the one act that grows principal.
    // A merge shrinks it and is unaffected.
    if matches!(request.direction, ClaimsConservationDirectionV1::Split) {
        request
            .admit_capacity(
                outstanding_sets,
                MarketPrincipalCapSetsV1::read(admitted.principal_cap_sets).to_sets(),
            )
            .map_err(|_| ClaimsSbfError::PrincipalCapacity)?;
    }
    Ok(AuthenticatedConservationMarketV1 {
        view: market,
        refunds_on_failure: admitted.refunds_on_failure,
    })
}

#[derive(Clone, Copy)]
struct AuthenticatedConservationMarketV1 {
    view: MarketViewV2,
    refunds_on_failure: bool,
}

/// What the escrow is on this act, and whether this Market seats one.
#[derive(Clone, Copy)]
struct ConservationEscrowV1 {
    seated: bool,
}

/// Authenticate the actor's Position, and the escrow the frame always names.
///
/// The escrow's law here is the complete-set gate's, not a second one: the
/// account offered must be the Position the MARKET derives (`FailureEscrow`),
/// and on a refunding Market it must already hold the whole failure column
/// (`FailureEscrowUnseated`) -- because the issuance shape is fixed at founding
/// and a routed act may maintain a refunding Market and may never convert a
/// categorical one.
#[inline(never)]
fn authenticate_positions_and_escrow(
    program_id: &Pubkey,
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    market: AuthenticatedConservationMarketV1,
) -> Result<ConservationEscrowV1, ProgramError> {
    authenticate_position(
        program_id,
        accounts.position,
        accounts.aggregate.key.to_bytes(),
        request.owner,
        market.view,
        Some(request.expected_position_revision),
    )?;
    let derived =
        FailureEscrowIdentityV1::derive(program_id, request.market, market.view.claim_count)
            .map_err(|_| ClaimsSbfError::FailureEscrow)?;
    let escrow_seeds =
        ProtocolPositionSeedsV2::new(accounts.aggregate.key.to_bytes(), derived.owner)
            .map_err(|_| ClaimsSbfError::FailureEscrow)?;
    if accounts.escrow_position.key
        != &Pubkey::find_program_address(&escrow_seeds.as_slices(), program_id).0
    {
        return Err(ClaimsSbfError::FailureEscrow.into());
    }
    if !market.refunds_on_failure {
        return Ok(ConservationEscrowV1 { seated: false });
    }
    if !accounts.escrow_position.is_writable {
        return Err(ClaimsSbfError::Accounts.into());
    }
    // The escrow's own Position is live and holds the whole failure column, or
    // this Market was not founded refunding and no routed act may convert it.
    let aggregate = accounts
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let failure_supply = market_supply(&aggregate, derived.failure_selector)
        .map_err(|_| ClaimsSbfError::Economic)?;
    drop(aggregate);
    if accounts.escrow_position.owner != program_id {
        return Err(ClaimsSbfError::FailureEscrowUnseated.into());
    }
    let escrow_data = accounts
        .escrow_position
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let seated = position_native(
        &escrow_data,
        market.view.claim_count,
        derived.failure_selector,
    )
    .map_err(|_| ClaimsSbfError::Identity)?;
    drop(escrow_data);
    if failure_supply == 0 || seated != failure_supply {
        return Err(ClaimsSbfError::FailureEscrowUnseated.into());
    }
    authenticate_position(
        program_id,
        accounts.escrow_position,
        accounts.aggregate.key.to_bytes(),
        derived.owner,
        market.view,
        None,
    )?;
    Ok(ConservationEscrowV1 { seated: true })
}

/// Authenticate one Position's address, ownership and identity joins.
///
/// `expected_revision` is `None` for the ESCROW, and that absence is stated
/// rather than faked: the request declares the actor's revision and says
/// nothing about the escrow's, so comparing the escrow's revision to itself
/// would be a check that cannot fail dressed as one that can.
fn authenticate_position(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    aggregate: [u8; 32],
    owner: [u8; 32],
    market: MarketViewV2,
    expected_revision: Option<u64>,
) -> Result<(), ProgramError> {
    let seeds =
        ProtocolPositionSeedsV2::new(aggregate, owner).map_err(|_| ClaimsSbfError::Identity)?;
    if account.owner != program_id
        || account.key != &Pubkey::find_program_address(&seeds.as_slices(), program_id).0
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let position = PositionViewV2::decode(&data).map_err(|_| ClaimsSbfError::Identity)?;
    if position.market_account != aggregate
        || position.owner != owner
        || position.basis_id != market.basis_id
        || position.claim_count != market.claim_count
        || expected_revision.is_some_and(|revision| position.revision != revision)
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

/// Run the kernel over the uniform complete-set vector this request states.
#[inline(never)]
fn apply_economics(
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    action: BasketAction,
    escrow: ConservationEscrowV1,
) -> Result<(), ProgramError> {
    let width = usize::try_from(request.claim_count)
        .ok()
        .and_then(|count| count.checked_mul(8))
        .ok_or(ClaimsSbfError::Instruction)?;
    let mut quantities = Vec::from_iter(core::iter::repeat_n(0_u8, width));
    request
        .write_uniform_quantities(&mut quantities)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    let mut aggregate = accounts
        .aggregate
        .try_borrow_mut_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let split = matches!(request.direction, ClaimsConservationDirectionV1::Split);
    // The escrow takes the slot the CATEGORICAL action of the same name leaves
    // empty: source for a refunding mint, destination for a refunding merge.
    // The kernel derives a merge's collateral payout from the SOURCE owner, so
    // putting the escrow there would pay the escrow instead of the holder who
    // burned the ordinary claims.
    let (source_present, destination_present) = if split {
        (escrow.seated, true)
    } else {
        (true, escrow.seated)
    };
    let holder_revision = request.expected_position_revision;
    let seated_revision = if escrow.seated {
        Some(escrow_revision_of(accounts, request.claim_count)?)
    } else {
        None
    };
    let (source_revision, destination_revision) = if split {
        (seated_revision, Some(holder_revision))
    } else {
        (Some(holder_revision), seated_revision)
    };
    let frame = BasketFrame {
        expected_market_revision: request.expected_market_revision,
        expected_source_revision: source_present.then_some(source_revision).flatten(),
        expected_destination_revision: destination_present
            .then_some(destination_revision)
            .flatten(),
        action,
        quantities: &quantities,
        quantity_multiplier: 1,
    };
    let holder = accounts.position;
    let seated = accounts.escrow_position;
    let payout = if escrow.seated {
        let mut holder_data = holder
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        let mut seated_data = seated
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        if split {
            execute_basket(
                &mut aggregate,
                Some(&mut seated_data),
                Some(&mut holder_data),
                frame,
            )
        } else {
            execute_basket(
                &mut aggregate,
                Some(&mut holder_data),
                Some(&mut seated_data),
                frame,
            )
        }
    } else {
        let mut holder_data = holder
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        if split {
            execute_basket(&mut aggregate, None, Some(&mut holder_data), frame)
        } else {
            execute_basket(&mut aggregate, Some(&mut holder_data), None, frame)
        }
    }
    .map_err(|_| ClaimsSbfError::Economic)?;
    // A merge's payout is in complete SETS and the Custody transfer moves
    // ATOMS. The conversion is `basis_scale`, and the request already states
    // the product; disagreeing with the kernel here is a conservation refusal
    // rather than a silently rounded transfer.
    if !split && payout.amount.checked_mul(request.basis_scale) != Some(request.collateral_atoms) {
        return Err(ClaimsSbfError::Economic.into());
    }
    Ok(())
}

/// The escrow's own current revision, read rather than stated.
///
/// The request declares the ACTOR's Position revision and says nothing about
/// the escrow's, because the escrow is derived rather than chosen and a caller
/// has no business pinning a revision on an account it does not control. The
/// optimism this route offers is therefore the actor's alone, which is the
/// honest scope: two conservation acts racing on one Market still collide at
/// the aggregate's revision.
///
/// A failure here is a REFUSAL, never a sentinel. `u64::MAX` is
/// `NO_POSITION_REVISION` and would have read as "no Position at all".
fn escrow_revision_of(
    accounts: ConservationAccounts<'_, '_>,
    claim_count: u32,
) -> Result<u64, ProgramError> {
    let data = accounts
        .escrow_position
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    position_revision(&data, claim_count).map_err(|_| ClaimsSbfError::Identity.into())
}

/// Build, authenticate and invoke the one Custody transfer this act owes.
///
/// EVERY WIDE VALUE ON THIS PATH IS BOXED, and that is not tidiness. The first
/// draft kept the conservation request, the derived `CustodyRequestV1` and its
/// encoded bytes as stack locals, and `cargo build-sbf` reported a 6,528-byte
/// frame against a 4,096-byte maximum with four "overwrites values in the
/// frame" diagnostics -- undefined behaviour at execution, caught by the build
/// rather than by a validator. The three helpers below are
/// `#[inline(never)]` for the same reason: each owns one wide value and none
/// of their frames is live at the same time.
#[inline(never)]
fn move_collateral(
    program_id: &Pubkey,
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
) -> Result<(), ProgramError> {
    let parent_digest = parent_request_digest(request)?;
    let custody = Box::new(
        request
            .custody_request(parent_digest)
            .map_err(|_| ClaimsSbfError::CustodyRequired)?,
    );
    let request_bytes = encode_custody_wire(accounts, request, parent_digest)?;
    authenticate_custody_frame(program_id, accounts, custody.as_ref(), &request_bytes)?;
    invoke_custody(program_id, accounts, custody.as_ref(), &request_bytes)
}

/// The digest of this request's own canonical bytes, which every derived
/// Custody request carries as its parent.
#[inline(never)]
fn parent_request_digest(request: ClaimsConservationRequestV1) -> Result<[u8; 32], ProgramError> {
    let bytes = Box::new(
        request
            .to_bytes()
            .map_err(|_| ClaimsSbfError::Instruction)?,
    );
    Ok(hash(bytes.as_slice()).to_bytes())
}

/// The exact wire Custody will be handed: delegated for a split, plain for a
/// merge.
#[inline(never)]
fn encode_custody_wire(
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    parent_digest: [u8; 32],
) -> Result<Vec<u8>, ProgramError> {
    if matches!(request.direction, ClaimsConservationDirectionV1::Split) {
        return encode_delegated_wire(accounts, request, parent_digest);
    }
    encode_plain_wire(request, parent_digest)
}

/// The delegated split wire, alone in its own frame.
///
/// Its own frame because the two arms' wide locals are otherwise both live in
/// one function and the sum is over the 4,096-byte maximum -- measured, not
/// guessed: the merged form built at 4,544 bytes.
#[inline(never)]
fn encode_delegated_wire(
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    parent_digest: [u8; 32],
) -> Result<Vec<u8>, ProgramError> {
    let delegated = Box::new(
        request
            .delegated_custody_request(parent_digest, accounts.custody_authority.key.to_bytes())
            .map_err(|_| ClaimsSbfError::CustodyRequired)?
            .encode()
            .map_err(|_| ClaimsSbfError::CustodyRequired)?,
    );
    Ok(delegated.to_vec())
}

/// The plain merge wire, alone in its own frame.
#[inline(never)]
fn encode_plain_wire(
    request: ClaimsConservationRequestV1,
    parent_digest: [u8; 32],
) -> Result<Vec<u8>, ProgramError> {
    let plain = Box::new(
        request
            .custody_request(parent_digest)
            .map_err(|_| ClaimsSbfError::CustodyRequired)?
            .to_bytes()
            .map_err(|_| ClaimsSbfError::CustodyRequired)?,
    );
    Ok(plain.to_vec())
}

#[inline(never)]
fn authenticate_custody_frame(
    program_id: &Pubkey,
    accounts: ConservationAccounts<'_, '_>,
    custody: &CustodyRequestV1,
    request_bytes: &[u8],
) -> Result<(), ProgramError> {
    let caller = CallerAuthoritySeedsV1::new(
        ContentId::new(custody.release_set).map_err(|_| ClaimsSbfError::Economic)?,
        custody.market,
        ExecutionRoleV1::Claims,
        custody.context,
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    let replay = CustodyReplaySeedsV1::from_request(*custody);
    let authority = CustodyAuthoritySeedsV1::from_request(*custody);
    let vault = CustodyVaultSeedsV1::from_request(*custody, true);
    // The pair, before anything is invoked: a request whose source and
    // destination are not this frame's two token accounts, in one order or the
    // other, is refused here rather than at the CPI boundary.
    transfer_pair(accounts, custody)?;
    if accounts.custody_caller_authority.key
        != &Pubkey::find_program_address(&caller.as_slices(), program_id).0
        || accounts.custody_replay.key
            != &Pubkey::find_program_address(&replay.as_slices(), accounts.custody_program.key).0
        || accounts.custody_authority.key
            != &Pubkey::find_program_address(&authority.as_slices(), accounts.custody_program.key).0
        || accounts.hoard_vault.key
            != &Pubkey::find_program_address(&vault.as_slices(), accounts.custody_program.key).0
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

/// The two token accounts Custody's Transfer frame names, IN ITS ORDER.
///
/// Coordinate 10 is `TransferSource` and 11 is `TransferDestination`
/// (`dclutch-custody`'s `frame_spec_v1`), and which of the Hoard and
/// the actor's own account stands at each depends on the DIRECTION. The
/// terminal payout next door is always a Hoard-to-holder transfer and can
/// therefore write the pair down; this route cannot, and writing it down
/// anyway is how a split would have handed Custody the Hoard as the account to
/// debit. The order is taken from the request Custody itself will decode, so
/// the frame and the wire cannot disagree.
fn transfer_pair<'accounts, 'info>(
    accounts: ConservationAccounts<'accounts, 'info>,
    custody: &CustodyRequestV1,
) -> Result<(&'accounts AccountInfo<'info>, &'accounts AccountInfo<'info>), ProgramError> {
    let hoard = accounts.hoard_vault.key.to_bytes();
    let external = accounts.external_collateral.key.to_bytes();
    if custody.source == external && custody.destination == hoard {
        return Ok((accounts.external_collateral, accounts.hoard_vault));
    }
    if custody.source == hoard && custody.destination == external {
        return Ok((accounts.hoard_vault, accounts.external_collateral));
    }
    Err(ClaimsSbfError::Identity.into())
}

#[inline(never)]
fn invoke_custody(
    program_id: &Pubkey,
    accounts: ConservationAccounts<'_, '_>,
    custody: &CustodyRequestV1,
    request_bytes: &[u8],
) -> Result<(), ProgramError> {
    let (source, destination) = transfer_pair(accounts, custody)?;
    let instruction = Instruction {
        program_id: *accounts.custody_program.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*accounts.custody_caller_authority.key, true),
            AccountMeta::new_readonly(*accounts.core_market.key, false),
            AccountMeta::new_readonly(*accounts.cache.key, false),
            AccountMeta::new_readonly(*accounts.registry.key, false),
            AccountMeta::new_readonly(*accounts.claims_program.key, false),
            AccountMeta::new_readonly(*accounts.claims_programdata.key, false),
            AccountMeta::new_readonly(*accounts.realm_record.key, false),
            AccountMeta::new_readonly(*accounts.realm_staging.key, false),
            AccountMeta::new(*accounts.custody_replay.key, false),
            AccountMeta::new_readonly(*accounts.collateral_mint.key, false),
            AccountMeta::new(*source.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*accounts.custody_authority.key, false),
            AccountMeta::new_readonly(*accounts.token_program.key, false),
        ]),
        data: request_bytes.to_vec(),
    };
    let caller = CallerAuthoritySeedsV1::new(
        ContentId::new(custody.release_set).map_err(|_| ClaimsSbfError::Economic)?,
        custody.market,
        ExecutionRoleV1::Claims,
        custody.context,
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| ClaimsSbfError::Economic)?;
    let bump = [Pubkey::find_program_address(&caller.as_slices(), program_id).1];
    let [domain, release, market, role, context, digest] = caller.as_slices();
    invoke_signed(
        &instruction,
        &[
            accounts.custody_caller_authority.clone(),
            accounts.core_market.clone(),
            accounts.cache.clone(),
            accounts.registry.clone(),
            accounts.claims_program.clone(),
            accounts.claims_programdata.clone(),
            accounts.realm_record.clone(),
            accounts.realm_staging.clone(),
            accounts.custody_replay.clone(),
            accounts.collateral_mint.clone(),
            source.clone(),
            destination.clone(),
            accounts.custody_authority.clone(),
            accounts.token_program.clone(),
            accounts.custody_program.clone(),
        ],
        &[&[domain, release, market, role, context, digest, &bump]],
    )
    .map_err(|_| ClaimsSbfError::CustodyRequired)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::conservation::{
        CLAIMS_CONSERVATION_REQUEST_MAGIC_V1, Error as ConservationError,
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn request(direction: ClaimsConservationDirectionV1) -> ClaimsConservationRequestV1 {
        // The balances move opposite ways, and the fixture has to say so: a
        // split takes 77 atoms out of the actor and puts them in the Hoard, a
        // merge does the reverse. A fixture that stated the split's balances
        // for both directions would have made every merge assertion pass or
        // fail for the wrong reason.
        let split = matches!(direction, ClaimsConservationDirectionV1::Split);
        let (pre_external, post_external) = if split { (100, 23) } else { (23, 100) };
        let (pre_hoard, post_hoard) = if split { (0, 77) } else { (77, 0) };
        ClaimsConservationRequestV1 {
            direction,
            realm: id(1),
            market: id(2),
            release_set: id(3),
            custody_context: id(4),
            aggregate: id(5),
            position: id(6),
            owner: id(7),
            external_collateral: id(8),
            hoard_vault: id(9),
            mint: id(10),
            token_program: id(11),
            claims_program: id(12),
            product_record_digest: id(13),
            linked_basis_record_digest: id(14),
            semantic_basis_id: id(15),
            generation: 1,
            quantity: 7,
            basis_scale: 11,
            collateral_atoms: 77,
            expected_market_revision: 4,
            expected_position_revision: 2,
            expected_custody_revision: 3,
            pre_external_amount: pre_external,
            post_external_amount: post_external,
            pre_hoard_amount: pre_hoard,
            post_hoard_amount: post_hoard,
            claim_count: 3,
        }
    }

    /// THE SLOT RULE, stated once and joined to the kernel's own authors.
    ///
    /// The escrow takes the slot the CATEGORICAL action of the same name
    /// leaves empty: source for a refunding mint, destination for a refunding
    /// merge. That is not a flourish -- the kernel derives a merge's collateral
    /// payout from the SOURCE owner, so putting the escrow there would pay the
    /// escrow instead of the holder who burned the ordinary claims. This test
    /// reads `escrow_is_source` off the kernel rather than restating it, so
    /// this route cannot drift from the law it is executing.
    #[test]
    fn the_escrow_takes_the_slot_the_categorical_action_leaves_empty() {
        for direction in [
            ClaimsConservationDirectionV1::Split,
            ClaimsConservationDirectionV1::Merge,
        ] {
            let categorical = conservation_basket_action_v1(direction, false);
            let refunding = conservation_basket_action_v1(direction, true);
            assert!(!categorical.is_refunding());
            assert!(refunding.is_refunding());
            let split = matches!(direction, ClaimsConservationDirectionV1::Split);
            assert_eq!(
                refunding.escrow_is_source(),
                split,
                "a refunding mint seats the escrow in the source and a refunding \
                 merge in the destination",
            );
        }
        assert_eq!(
            conservation_basket_action_v1(ClaimsConservationDirectionV1::Split, false),
            BasketAction::MintCompleteSet,
        );
        assert_eq!(
            conservation_basket_action_v1(ClaimsConservationDirectionV1::Merge, false),
            BasketAction::MergeCompleteSet,
        );
    }

    /// A SPLIT THAT MOVES NO COLLATERAL REFUSES, in the contract this route
    /// dispatches, before any account is touched.
    ///
    /// This is the defect the generic migration-only route has and cannot lose:
    /// its mint credits the aggregate's Hoard SCALAR and transfers no atoms, so
    /// claims come into existence against a Hoard that received nothing. Here
    /// the two halves are derived from ONE request, so a split whose stated
    /// external balance does not fall by exactly `quantity * basis_scale` is
    /// not a split.
    #[test]
    fn a_split_that_moves_no_collateral_refuses() {
        let mut hostile = request(ClaimsConservationDirectionV1::Split);
        hostile.post_external_amount = hostile.pre_external_amount;
        assert_eq!(
            hostile.validate().unwrap_err(),
            ConservationError::ExternalBalanceMismatch,
        );
        let mut unbacked = request(ClaimsConservationDirectionV1::Split);
        unbacked.post_hoard_amount = unbacked.pre_hoard_amount;
        assert_eq!(
            unbacked.validate().unwrap_err(),
            ConservationError::HoardBalanceMismatch,
        );
        // And the positive control, so the two refusals above are not a gate
        // that refuses everything.
        request(ClaimsConservationDirectionV1::Split)
            .validate()
            .expect("an exact split is admitted");
    }

    /// A stated collateral that is not `quantity * basis_scale` refuses.
    ///
    /// The multiplication is EXACT and has no rounding boundary; an inexact
    /// result is a refusal, not a remainder.
    #[test]
    fn collateral_that_is_not_the_exact_product_refuses() {
        let mut hostile = request(ClaimsConservationDirectionV1::Merge);
        hostile.collateral_atoms = 76;
        assert_eq!(
            hostile.validate().unwrap_err(),
            ConservationError::CollateralMismatch,
        );
    }

    /// WHICH TOKEN ACCOUNT CUSTODY DEBITS depends on the direction, and this
    /// route may not write the pair down.
    ///
    /// Custody's Transfer frame names coordinate 10 `TransferSource` and 11
    /// `TransferDestination`. The terminal payout next door is always a
    /// Hoard-to-holder transfer and can therefore hardcode the pair; the first
    /// draft of this route copied that and would have handed Custody the HOARD
    /// as the account to debit on a split -- a working merge and a split that
    /// drains the Market. `transfer_pair` now orders the two by reading the
    /// request Custody itself will decode, and this is that request's own
    /// answer for both directions.
    #[test]
    fn a_split_debits_the_actor_and_a_merge_debits_the_hoard() {
        let digest = id(0x9d);
        let split = request(ClaimsConservationDirectionV1::Split)
            .custody_request(digest)
            .expect("split custody request");
        assert_eq!(
            split.source,
            id(8),
            "a split debits the actor's own account"
        );
        assert_eq!(split.destination, id(9), "a split credits the Hoard");
        let merge = request(ClaimsConservationDirectionV1::Merge)
            .custody_request(digest)
            .expect("merge custody request");
        assert_eq!(merge.source, id(9), "a merge debits the Hoard");
        assert_eq!(
            merge.destination,
            id(8),
            "a merge credits the actor's own account"
        );
    }

    /// The dispatch predicate selects this family and nothing adjacent.
    #[test]
    fn the_dispatch_predicate_is_exact() {
        let bytes = request(ClaimsConservationDirectionV1::Split)
            .to_bytes()
            .expect("canonical bytes");
        assert!(is_claims_conservation_v1(&bytes));
        let mut wrong_magic = bytes;
        wrong_magic[0] ^= 0xff;
        assert!(!is_claims_conservation_v1(&wrong_magic));
        assert!(!is_claims_conservation_v1(
            &bytes[..CLAIMS_CONSERVATION_REQUEST_BYTES_V1 - 1]
        ));
        assert!(!is_claims_conservation_v1(
            CLAIMS_CONSERVATION_REQUEST_MAGIC_V1.as_slice()
        ));
    }
}
