//! Split and merge as USER ACTS that move collateral, on one account family.
//!
//! Decision 0029 item 5 ruled BUILD: *"so a holder can reshape a position
//! without a counterparty"*. This is the outer route for `DCLCNS01`, and the
//! second draft of it. The first (CLAIMS-17) read every subject account with
//! decoders from two disjoint families -- the LBV2 aggregate `founding_v5`
//! writes (`DCLLBM02`, one supplies vector, no Hoard scalar) and the economic
//! slice's `DCLTEMK2` (three vectors, a Hoard scalar at 32) -- at eight
//! cross-family call sites, so no byte string satisfied both readers and
//! CLAIMS-18 proved on the shipped ELF that it could not execute.
//!
//! # The ruling this executes
//!
//! *An LBV2 Market's outstanding principal lives in the Custody HoardPrincipal
//! vault (the token account), not in a header scalar; ONE LBV2 complete-set
//! executor shared by signed_delta, affine_batch and conservation*
//! (orchestrator, 2026-09-05, provisional). So this route reads the aggregate
//! and both Positions as LBV2 and nothing else; reads the principal off the
//! vault's balance through Custody's own token reader; and moves claims
//! through `dclutch_claims::complete_set_v1`, the executor the two batch
//! routes now call for their coordinate writes too.
//!
//! # What one account proves
//!
//! The first draft walked the whole Product graph (four record pairs, about
//! 40,000 CU) to learn three things: that the Market refunds on failure, its
//! basis scale, and its width. PROGRAMS-17E found that the linked basis
//! record's own bytes prove all three -- `semantic_basis_id_v3` hashes exactly
//! the kind, width and payout scale `categorical_refunds_on_failure_v3` reads,
//! and the aggregate's `basis_id` is that hash -- so a record that reproduces
//! the aggregate's `basis_id` cannot disagree with the Market about any of
//! them. The Product record's digest is joined by Core's own persisted
//! identity (`authenticate_core_market_v3`), which also carries the phase and
//! the principal cap. The frame therefore drops the seven Product-graph
//! accounts: twenty-nine became twenty-one.
//!
//! # The two shapes, and why the wire does not distinguish them
//!
//! A categorical Market's complete set lives in one Position. A refunding
//! Market's lives in two: the ordinary coordinates with the holder, the
//! failure coordinate with the Market's own escrow (decision 0025 item 2, and
//! the merge amendment that redefined the set over the ordinary coordinates).
//! The wire says nothing about which -- the RECORD does -- and the executor
//! seats the coordinates itself from the sole author of the failure selector.
//!
//! # The order, both ways
//!
//! A SPLIT moves the collateral first and mints second: claims must never
//! exist against a vault that has not yet received their backing, even inside
//! one instruction. A MERGE burns first and pays second, for the mirror
//! reason. Both are atomic; the candidates are computed before either
//! chain-visible effect, and committed last.
//!
//! # Why a split needs the delegated Custody wire
//!
//! A split debits the actor's OWN external token account, and Custody's V1
//! `Transfer` refuses an `External` source outright, so an apparently correct
//! balance delta cannot leave hidden delegated spending authority behind. The
//! actor approves exactly `collateral_atoms` to the Custody transfer authority
//! in the same transaction; the single transfer consumes all of it and revokes
//! the delegation (`allowance_before == collateral_atoms`,
//! `allowance_after == 0`).
//!
//! # The pair, taken from the request Custody decodes
//!
//! Which of the vault and the actor's account Custody debits depends on the
//! DIRECTION. The terminal payout next door is always vault-to-holder and can
//! write the pair down; this route cannot, and the first draft's
//! `authenticate_custody_frame` derived the vault's seeds from the request's
//! SOURCE side -- which on a split is the External side -- so every split
//! would have refused `Identity` at that stage had CLAIMS-18 reached it.
//! [`transfer_pair`] orders the two by reading the request Custody itself will
//! decode, and the vault's seeds come from the contract's direction-free
//! [`ClaimsConservationRequestV1::hoard_vault_seeds`].

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};

use dclutch_claims::complete_set_v1::{
    CompleteSetActV1, CompleteSetDirectionV1, CompleteSetErrorV1, apply_complete_set_v1,
    principal_v1,
};
use dclutch_claims::conservation::{
    CLAIMS_CONSERVATION_REQUEST_BYTES_V1, ClaimsConservationDirectionV1,
    ClaimsConservationRequestV1,
};
use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_BUMP_OFFSET_V2, LIABILITY_BASIS_MARKET_SEED_V2,
    LIABILITY_BASIS_POSITION_BUMP_OFFSET_V2, LiabilityBasisMarketViewV2 as MarketViewV2,
    LiabilityBasisPositionViewV2 as PositionViewV2,
};
use dclutch_claims::protocol_position_v2::ProtocolPositionSeedsV2;
use dclutch_core_contract::ContentId;
use dclutch_custody::token_svm::TokenAccount;
use dclutch_custody::{CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyRequestV1};
use dclutch_market::{CoreState, STATE_BYTES};
use dclutch_product::payoff::runtime_v3::{ProductBasisV3, semantic_basis_id_v3};
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

use crate::affine_batch_v2::authenticate_core_market_v3;
use crate::claims_cu_checkpoint;
use crate::market_admission_v1::CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1;
use crate::{ClaimsSbfError, FailureEscrowIdentityV1};

/// Stable conservation refusal, in Claims' `0x5300` sub-band.
///
/// Each variant is one accusation. The two escrow refusals are deliberately
/// NOT here: a Position offered as the escrow that is not the Market's own is
/// [`ClaimsSbfError::FailureEscrow`], and the Market's own escrow not holding
/// the failure column is [`ClaimsSbfError::FailureEscrowUnseated`] -- the
/// codes the founding, the batch gate and the closure already raise for the
/// same two mistakes, so a reader meets one code per accusation across the
/// four routes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimsConservationSbfErrorV1 {
    /// The request bytes refused the `DCLCNS01` codec or its own arithmetic.
    Instruction = 0x5300,
    /// Account count, privileges, owners, executables, or the request's
    /// account coordinates did not match the frame.
    Accounts = 0x5301,
    /// The aggregate or a Position is not the derived account, does not join
    /// the request, or is not at the pinned revision; or a token account is
    /// not the mint's under the owner the request names.
    Identity = 0x5302,
    /// The linked basis record is not this Market's, disagrees with the
    /// request's basis scale or digest, or the Core join refused.
    ProductBasis = 0x5303,
    /// The Core Market is not Open.
    Phase = 0x5304,
    /// A split would grow outstanding principal past the Market's carried
    /// manipulation-capacity cap.
    PrincipalCapacity = 0x5305,
    /// A stated pre-balance -- the actor's or the vault's -- is not what the
    /// account holds.
    Balances = 0x5306,
    /// The vault does not back the outstanding supply at the basis scale:
    /// L4's LBV2 form, refused before any act.
    Backing = 0x5307,
    /// A merge found less than `quantity` at a coordinate it burns: the holder
    /// does not hold a complete set.
    Holding = 0x5308,
    /// The complete-set arithmetic overflowed, or a revision could not
    /// advance, or a coordinate did not fit its candidate.
    Candidate = 0x5309,
    /// The derived Custody request or its wire could not be constructed.
    CustodyWire = 0x530A,
    /// Custody's receipt was absent or the token balances after the transfer
    /// are not the request's stated poststate.
    Receipt = 0x530B,
    /// The candidates could not all be borrowed and committed last.
    Commit = 0x530C,
}

dclutch_refusal_registry::pin_refusal_band!(
    ClaimsConservationSbfErrorV1,
    dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x300,
    [
        Instruction,
        Accounts,
        Identity,
        ProductBasis,
        Phase,
        PrincipalCapacity,
        Balances,
        Backing,
        Holding,
        Candidate,
        CustodyWire,
        Receipt,
        Commit,
    ]
);

/// The frame's coordinates, owned by the contract crate and re-exported here
/// so the operator's builder and this parser read one table.
pub use dclutch_claims::conservation::frame_v1::{
    AGGREGATE, BASIS_RECORD, CACHE, CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1, CLAIMS_PROGRAM,
    CLAIMS_PROGRAMDATA, COLLATERAL_MINT, CORE_MARKET, CORE_PROGRAM, CUSTODY_AUTHORITY,
    CUSTODY_CALLER_AUTHORITY, CUSTODY_PROGRAM, CUSTODY_REPLAY, ESCROW_POSITION,
    EXTERNAL_COLLATERAL, HOARD_VAULT, OWNER, POSITION, REALM_RECORD, REALM_STAGING, REGISTRY,
    TOKEN_PROGRAM,
};

#[derive(Clone, Copy)]
struct ConservationAccounts<'accounts, 'info> {
    owner: &'accounts AccountInfo<'info>,
    aggregate: &'accounts AccountInfo<'info>,
    position: &'accounts AccountInfo<'info>,
    escrow_position: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    basis_record: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
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
            return Err(ClaimsConservationSbfErrorV1::Accounts.into());
        }
        let at = |index: usize| -> Result<&'accounts AccountInfo<'info>, ProgramError> {
            accounts
                .get(index)
                .ok_or(ClaimsConservationSbfErrorV1::Accounts.into())
        };
        Ok(Self {
            owner: at(OWNER)?,
            aggregate: at(AGGREGATE)?,
            position: at(POSITION)?,
            escrow_position: at(ESCROW_POSITION)?,
            core_market: at(CORE_MARKET)?,
            basis_record: at(BASIS_RECORD)?,
            cache: at(CACHE)?,
            registry: at(REGISTRY)?,
            claims_program: at(CLAIMS_PROGRAM)?,
            claims_programdata: at(CLAIMS_PROGRAMDATA)?,
            core_program: at(CORE_PROGRAM)?,
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
            == Some(dclutch_claims::conservation::CLAIMS_CONSERVATION_REQUEST_MAGIC_V1.as_slice())
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
        .map_err(|_| ClaimsConservationSbfErrorV1::Instruction)?;
    let accounts = ConservationAccounts::parse(account_infos)?;
    authenticate_privileges(program_id, accounts, request)?;
    claims_cu_checkpoint!("conserve-privileges");
    let market = authenticate_aggregate(program_id, accounts, request)?;
    let basis = authenticate_basis_record(accounts, request, market)?;
    claims_cu_checkpoint!("conserve-records");
    let principal_cap_sets = authenticate_core(accounts, request, market)?;
    claims_cu_checkpoint!("conserve-core");
    let escrow = authenticate_positions_and_escrow(program_id, accounts, request, market, basis)?;
    claims_cu_checkpoint!("conserve-positions");
    let prestate = authenticate_balances_and_backing(accounts, request, principal_cap_sets)?;
    claims_cu_checkpoint!("conserve-backing");

    let candidates = build_candidates(accounts, request, escrow)?;
    claims_cu_checkpoint!("conserve-candidates");

    // A SPLIT moves the collateral before the claims exist; a MERGE burns the
    // claims before the collateral leaves. Neither ordering is observable from
    // outside the instruction, and both are the readable one.
    if matches!(request.direction, ClaimsConservationDirectionV1::Split) {
        move_collateral(program_id, accounts, request)?;
        claims_cu_checkpoint!("conserve-collateral");
        verify_balances_after(accounts, request, prestate)?;
        commit_candidates(accounts, &candidates)?;
    } else {
        commit_candidates(accounts, &candidates)?;
        claims_cu_checkpoint!("conserve-economics");
        move_collateral(program_id, accounts, request)?;
        verify_balances_after(accounts, request, prestate)?;
    }
    claims_cu_checkpoint!("conserve-done");
    Ok(())
}

fn authenticate_privileges(
    program_id: &Pubkey,
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
) -> Result<(), ProgramError> {
    // The owner's writability carries no authority and is not pinned: a
    // wallet that authorizes its own act pays the transaction's fee, and a fee
    // payer is a writable signer. Signer stays required, executable refused.
    if !accounts.owner.is_signer
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
        || !accounts.token_program.executable
        || accounts.aggregate.key.to_bytes() != request.aggregate
        || accounts.position.key.to_bytes() != request.position
        || accounts.hoard_vault.key.to_bytes() != request.hoard_vault
        || accounts.external_collateral.key.to_bytes() != request.external_collateral
        || accounts.collateral_mint.key.to_bytes() != request.mint
        || accounts.token_program.key.to_bytes() != request.token_program
        || accounts.core_market.key.to_bytes() != request.market
    {
        return Err(ClaimsConservationSbfErrorV1::Accounts.into());
    }
    for readonly in [
        accounts.core_market,
        accounts.basis_record,
        accounts.cache,
        accounts.registry,
        accounts.claims_program,
        accounts.claims_programdata,
        accounts.core_program,
        accounts.custody_caller_authority,
        accounts.custody_program,
        accounts.collateral_mint,
        accounts.token_program,
        accounts.custody_authority,
        accounts.realm_record,
        accounts.realm_staging,
    ] {
        if readonly.is_signer || readonly.is_writable {
            return Err(ClaimsConservationSbfErrorV1::Accounts.into());
        }
    }
    Ok(())
}

/// One account body's own recorded bump. Zero is unrecorded and its reader
/// searches, so a body written before the byte existed is no worse off.
fn recorded_bump(body: &[u8], offset: usize) -> u8 {
    body.get(offset).copied().unwrap_or(0)
}

/// Reproduce one address from a recorded bump, degrading to the search.
///
/// Reading a hint must not be able to refuse: an unrecorded bump, or one whose
/// derivation fails outright, falls back to `find_program_address`. Only the
/// address equality the caller already had can refuse.
fn derive_hinted(seeds: &[&[u8]], program_id: &Pubkey, hint: u8) -> Pubkey {
    if hint != 0 {
        let bump = [hint];
        let mut with_bump: Vec<&[u8]> = Vec::with_capacity(seeds.len().saturating_add(1));
        with_bump.extend_from_slice(seeds);
        with_bump.push(&bump);
        if let Ok(address) = Pubkey::create_program_address(&with_bump, program_id) {
            return address;
        }
    }
    Pubkey::find_program_address(seeds, program_id).0
}

/// Authenticate the LBV2 aggregate: owner, derived address, every identity
/// join to the request, and the pinned revision.
#[inline(never)]
fn authenticate_aggregate(
    program_id: &Pubkey,
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
) -> Result<MarketViewV2, ProgramError> {
    if accounts.aggregate.owner != program_id {
        return Err(ClaimsConservationSbfErrorV1::Identity.into());
    }
    let data = accounts
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?;
    let market = MarketViewV2::decode(&data).map_err(|_| ClaimsConservationSbfErrorV1::Identity)?;
    // Reproduced from the bump this program recorded when it founded the
    // aggregate, not searched for.
    let expected = derive_hinted(
        &[
            LIABILITY_BASIS_MARKET_SEED_V2,
            market.logical_market.as_slice(),
        ],
        program_id,
        recorded_bump(&data, LIABILITY_BASIS_MARKET_BUMP_OFFSET_V2),
    );
    if accounts.aggregate.key != &expected
        || market.logical_market != request.market
        || market.release_set != request.release_set
        || market.registry_program != accounts.registry.key.to_bytes()
        || market.basis_id != request.semantic_basis_id
        || market.custody_context != request.custody_context
        || market.realm_id != request.realm
        || market.generation != request.generation
        || market.claim_count != request.claim_count
        || market.revision != request.expected_market_revision
    {
        return Err(ClaimsConservationSbfErrorV1::Identity.into());
    }
    Ok(market)
}

/// What the record proves about this Market.
#[derive(Clone, Copy)]
struct AuthenticatedBasisV1 {
    refunds_on_failure: bool,
}

/// ONE ACCOUNT PROVES THE RECORD.
///
/// The request pins the record's exact bytes by digest; the record's semantic
/// identity reproduces the aggregate's `basis_id`, which founding wrote from
/// the record Core authenticated; its width is the aggregate's; and its payout
/// scale is the basis scale the request states and every collateral figure
/// was computed at. There is no Product-graph walk: `semantic_basis_id_v3`'s
/// preimage carries exactly the kind, width and scale
/// `categorical_refunds_on_failure_v3` reads, so a record that reproduces the
/// id cannot disagree with the Market about whether it refunds.
#[inline(never)]
fn authenticate_basis_record(
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    market: MarketViewV2,
) -> Result<AuthenticatedBasisV1, ProgramError> {
    let bytes = accounts
        .basis_record
        .try_borrow_data()
        .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?;
    if hash(&bytes).to_bytes() != request.linked_basis_record_digest {
        return Err(ClaimsConservationSbfErrorV1::ProductBasis.into());
    }
    let semantic =
        semantic_basis_id_v3(&bytes).map_err(|_| ClaimsConservationSbfErrorV1::ProductBasis)?;
    let basis =
        ProductBasisV3::decode(&bytes).map_err(|_| ClaimsConservationSbfErrorV1::ProductBasis)?;
    if semantic != market.basis_id
        || basis.basis_width() != market.claim_count
        || basis.payout_scale() != request.basis_scale
    {
        return Err(ClaimsConservationSbfErrorV1::ProductBasis.into());
    }
    Ok(AuthenticatedBasisV1 {
        refunds_on_failure: basis.refunds_on_failure(),
    })
}

/// The Core join: phase by name, then Core's own identity and the sole
/// runtime principal cap.
///
/// The phase is read here BEFORE the shared join, because the join folds a
/// wrong phase into `ProductBasis` and a reader who tried to split on a
/// Terminal Market should be told that, not sent to hunt a substituted
/// record.
#[inline(never)]
fn authenticate_core(
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    market: MarketViewV2,
) -> Result<u64, ProgramError> {
    {
        let core_data = accounts
            .core_market
            .try_borrow_data()
            .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?;
        if accounts.core_market.owner != accounts.core_program.key
            || accounts.core_market.data_len() != STATE_BYTES
        {
            return Err(ClaimsConservationSbfErrorV1::ProductBasis.into());
        }
        let core =
            CoreState::decode(&core_data).map_err(|_| ClaimsConservationSbfErrorV1::ProductBasis)?;
        if !CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(core.phase) {
            return Err(ClaimsConservationSbfErrorV1::Phase.into());
        }
    }
    authenticate_core_market_v3(
        accounts.core_market,
        accounts.core_program,
        accounts.registry,
        market,
        request.product_record_digest,
        CLAIMS_OPEN_MARKET_ADMISSIBLE_PRESTATES_V1,
    )
    .map_err(|_| ClaimsConservationSbfErrorV1::ProductBasis.into())
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
    market: MarketViewV2,
    basis: AuthenticatedBasisV1,
) -> Result<ConservationEscrowV1, ProgramError> {
    authenticate_position(
        program_id,
        accounts.position,
        accounts.aggregate.key.to_bytes(),
        request.owner,
        market,
        Some(request.expected_position_revision),
    )?;
    let derived =
        FailureEscrowIdentityV1::derive(program_id, request.market, market.claim_count)
            .map_err(|_| ClaimsSbfError::FailureEscrow)?;
    let escrow_seeds =
        ProtocolPositionSeedsV2::new(accounts.aggregate.key.to_bytes(), derived.owner)
            .map_err(|_| ClaimsSbfError::FailureEscrow)?;
    if accounts.escrow_position.key
        != &Pubkey::find_program_address(&escrow_seeds.as_slices(), program_id).0
    {
        return Err(ClaimsSbfError::FailureEscrow.into());
    }
    if !basis.refunds_on_failure {
        return Ok(ConservationEscrowV1 { seated: false });
    }
    if !accounts.escrow_position.is_writable {
        return Err(ClaimsConservationSbfErrorV1::Accounts.into());
    }
    // The escrow's own Position is live and holds the whole failure column, or
    // this Market was not founded refunding and no routed act may convert it.
    let failure_supply = {
        let aggregate = accounts
            .aggregate
            .try_borrow_data()
            .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?;
        market
            .supply(&aggregate, derived.failure_selector)
            .map_err(|_| ClaimsConservationSbfErrorV1::Identity)?
    };
    if accounts.escrow_position.owner != program_id {
        return Err(ClaimsSbfError::FailureEscrowUnseated.into());
    }
    let seated = {
        let escrow_data = accounts
            .escrow_position
            .try_borrow_data()
            .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?;
        let view = PositionViewV2::decode(&escrow_data)
            .map_err(|_| ClaimsSbfError::FailureEscrowUnseated)?;
        view.balance(&escrow_data, derived.failure_selector)
            .map_err(|_| ClaimsSbfError::FailureEscrowUnseated)?
    };
    if failure_supply == 0 || seated != failure_supply {
        return Err(ClaimsSbfError::FailureEscrowUnseated.into());
    }
    authenticate_position(
        program_id,
        accounts.escrow_position,
        accounts.aggregate.key.to_bytes(),
        derived.owner,
        market,
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
    let seeds = ProtocolPositionSeedsV2::new(aggregate, owner)
        .map_err(|_| ClaimsConservationSbfErrorV1::Identity)?;
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?;
    let expected = derive_hinted(
        &seeds.as_slices(),
        program_id,
        recorded_bump(&data, LIABILITY_BASIS_POSITION_BUMP_OFFSET_V2),
    );
    if account.owner != program_id || account.key != &expected {
        return Err(ClaimsConservationSbfErrorV1::Identity.into());
    }
    let position =
        PositionViewV2::decode(&data).map_err(|_| ClaimsConservationSbfErrorV1::Identity)?;
    if position.market_account != aggregate
        || position.owner != owner
        || position.basis_id != market.basis_id
        || position.claim_count != market.claim_count
        || expected_revision.is_some_and(|revision| position.revision != revision)
    {
        return Err(ClaimsConservationSbfErrorV1::Identity.into());
    }
    Ok(())
}

/// The two token balances before the act, read rather than trusted.
#[derive(Clone, Copy)]
struct TokenPrestateV1 {
    vault_atoms: u64,
    external_atoms: u64,
}

/// Read one token account's balance through Custody's own reader.
///
/// The mint must be the Realm's and the owner the one the request names for
/// that side -- the Custody transfer authority for the vault, the actor for
/// their own account -- so a split can no more credit a stranger's vault than
/// a merge can pay a stranger's wallet.
fn token_balance(
    account: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    mint: [u8; 32],
    owner: [u8; 32],
) -> Result<u64, ProgramError> {
    if account.owner != token_program.key {
        return Err(ClaimsConservationSbfErrorV1::Accounts.into());
    }
    let bytes = account
        .try_borrow_data()
        .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?;
    let token = TokenAccount::parse_base_or_immutable_owner(&bytes)
        .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?;
    if token.mint != mint || token.owner != owner {
        return Err(ClaimsConservationSbfErrorV1::Identity.into());
    }
    Ok(token.amount)
}

/// Read both balances, hold them to the request's stated prestate, and prove
/// the vault backs the outstanding supply before anything moves.
///
/// The principal is READ OFF THE VAULT, which is the ruling: the aggregate
/// carries no scalar to consult. A split is then held to Core's sole runtime
/// principal cap at the one act that grows principal; a merge shrinks it and
/// is unaffected.
#[inline(never)]
fn authenticate_balances_and_backing(
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    principal_cap_sets: u64,
) -> Result<TokenPrestateV1, ProgramError> {
    let vault_atoms = token_balance(
        accounts.hoard_vault,
        accounts.token_program,
        request.mint,
        accounts.custody_authority.key.to_bytes(),
    )?;
    let external_atoms = token_balance(
        accounts.external_collateral,
        accounts.token_program,
        request.mint,
        request.owner,
    )?;
    if vault_atoms != request.pre_hoard_amount || external_atoms != request.pre_external_amount {
        return Err(ClaimsConservationSbfErrorV1::Balances.into());
    }
    let principal = {
        let aggregate = accounts
            .aggregate
            .try_borrow_data()
            .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?;
        principal_v1(&aggregate, vault_atoms, request.basis_scale).map_err(|error| match error {
            CompleteSetErrorV1::Backing => ClaimsConservationSbfErrorV1::Backing,
            CompleteSetErrorV1::Overflow => ClaimsConservationSbfErrorV1::Candidate,
            _ => ClaimsConservationSbfErrorV1::Identity,
        })?
    };
    if matches!(request.direction, ClaimsConservationDirectionV1::Split) {
        request
            .admit_capacity(
                principal.outstanding_sets,
                MarketPrincipalCapSetsV1::read(principal_cap_sets).to_sets(),
            )
            .map_err(|_| ClaimsConservationSbfErrorV1::PrincipalCapacity)?;
    }
    Ok(TokenPrestateV1 {
        vault_atoms,
        external_atoms,
    })
}

/// The candidate bodies of every account the act writes.
struct CandidatesV1 {
    aggregate: Vec<u8>,
    holder: Vec<u8>,
    escrow: Option<Vec<u8>>,
}

/// Run the executor over copies of the three bodies.
///
/// The executor writes as it goes and rolls nothing back, so it is handed
/// copies and the copies are committed last, whole or not at all -- the same
/// discipline the batch routes' `commit_candidates` applies.
#[inline(never)]
fn build_candidates(
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    escrow: ConservationEscrowV1,
) -> Result<CandidatesV1, ProgramError> {
    let mut aggregate = accounts
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?
        .to_vec();
    let mut holder = accounts
        .position
        .try_borrow_data()
        .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?
        .to_vec();
    let mut seated = if escrow.seated {
        Some(
            accounts
                .escrow_position
                .try_borrow_data()
                .map_err(|_| ClaimsConservationSbfErrorV1::Accounts)?
                .to_vec(),
        )
    } else {
        None
    };
    let direction = match request.direction {
        ClaimsConservationDirectionV1::Split => CompleteSetDirectionV1::Mint,
        ClaimsConservationDirectionV1::Merge => CompleteSetDirectionV1::Merge,
    };
    apply_complete_set_v1(
        &mut aggregate,
        &mut holder,
        seated.as_deref_mut(),
        CompleteSetActV1 {
            direction,
            quantity: request.quantity,
            refunding: escrow.seated,
        },
    )
    .map_err(|error| -> ProgramError {
        match error {
            CompleteSetErrorV1::Holding => ClaimsConservationSbfErrorV1::Holding.into(),
            CompleteSetErrorV1::Overflow
            | CompleteSetErrorV1::Coordinate
            | CompleteSetErrorV1::Revision => ClaimsConservationSbfErrorV1::Candidate.into(),
            CompleteSetErrorV1::Quantity => ClaimsConservationSbfErrorV1::Instruction.into(),
            CompleteSetErrorV1::Width
            | CompleteSetErrorV1::EscrowRequired
            | CompleteSetErrorV1::EscrowForbidden => ClaimsSbfError::FailureEscrow.into(),
            CompleteSetErrorV1::Backing => ClaimsConservationSbfErrorV1::Backing.into(),
            CompleteSetErrorV1::Aggregate(_)
            | CompleteSetErrorV1::Position(_)
            | CompleteSetErrorV1::Join => ClaimsConservationSbfErrorV1::Identity.into(),
        }
    })?;
    Ok(CandidatesV1 {
        aggregate,
        holder,
        escrow: seated,
    })
}

#[inline(never)]
fn commit_candidates(
    accounts: ConservationAccounts<'_, '_>,
    candidates: &CandidatesV1,
) -> Result<(), ProgramError> {
    let mut aggregate = accounts
        .aggregate
        .try_borrow_mut_data()
        .map_err(|_| ClaimsConservationSbfErrorV1::Commit)?;
    let mut holder = accounts
        .position
        .try_borrow_mut_data()
        .map_err(|_| ClaimsConservationSbfErrorV1::Commit)?;
    if aggregate.len() != candidates.aggregate.len() || holder.len() != candidates.holder.len() {
        return Err(ClaimsConservationSbfErrorV1::Commit.into());
    }
    if let Some(candidate) = candidates.escrow.as_ref() {
        let mut escrow = accounts
            .escrow_position
            .try_borrow_mut_data()
            .map_err(|_| ClaimsConservationSbfErrorV1::Commit)?;
        if escrow.len() != candidate.len() {
            return Err(ClaimsConservationSbfErrorV1::Commit.into());
        }
        escrow.copy_from_slice(candidate);
    }
    aggregate.copy_from_slice(&candidates.aggregate);
    holder.copy_from_slice(&candidates.holder);
    Ok(())
}

/// After Custody has moved the atoms: both balances are the request's stated
/// poststate, read back through the same reader.
///
/// Custody's own postcondition already holds the delta; this holds the ABSOLUTE
/// figures the actor signed for, so a request whose stated prestate was true
/// and whose stated poststate is not cannot commit.
#[inline(never)]
fn verify_balances_after(
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    prestate: TokenPrestateV1,
) -> Result<(), ProgramError> {
    let vault_atoms = token_balance(
        accounts.hoard_vault,
        accounts.token_program,
        request.mint,
        accounts.custody_authority.key.to_bytes(),
    )?;
    let external_atoms = token_balance(
        accounts.external_collateral,
        accounts.token_program,
        request.mint,
        request.owner,
    )?;
    let split = matches!(request.direction, ClaimsConservationDirectionV1::Split);
    let (expected_vault, expected_external) = if split {
        (
            prestate.vault_atoms.checked_add(request.collateral_atoms),
            prestate.external_atoms.checked_sub(request.collateral_atoms),
        )
    } else {
        (
            prestate.vault_atoms.checked_sub(request.collateral_atoms),
            prestate.external_atoms.checked_add(request.collateral_atoms),
        )
    };
    if Some(vault_atoms) != expected_vault
        || Some(external_atoms) != expected_external
        || vault_atoms != request.post_hoard_amount
        || external_atoms != request.post_external_amount
    {
        return Err(ClaimsConservationSbfErrorV1::Receipt.into());
    }
    Ok(())
}

/// Build, authenticate and invoke the one Custody transfer this act owes.
///
/// EVERY WIDE VALUE ON THIS PATH IS BOXED, and that is not tidiness. The first
/// draft kept the conservation request, the derived `CustodyRequestV1` and its
/// encoded bytes as stack locals, and `cargo build-sbf` reported a 6,528-byte
/// frame against a 4,096-byte maximum. The helpers below are `#[inline(never)]`
/// for the same reason: each owns one wide value and none of their frames is
/// live at the same time.
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
            .map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?,
    );
    let request_bytes = encode_custody_wire(accounts, request, parent_digest)?;
    authenticate_custody_frame(program_id, accounts, request, custody.as_ref(), &request_bytes)?;
    invoke_custody(program_id, accounts, custody.as_ref(), &request_bytes)
}

/// The digest of this request's own canonical bytes, which every derived
/// Custody request carries as its parent.
#[inline(never)]
fn parent_request_digest(request: ClaimsConservationRequestV1) -> Result<[u8; 32], ProgramError> {
    let bytes = Box::new(
        request
            .to_bytes()
            .map_err(|_| ClaimsConservationSbfErrorV1::Instruction)?,
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

/// The delegated split wire, alone in its own frame (the merged form built at
/// 4,544 bytes against the 4,096-byte maximum).
#[inline(never)]
fn encode_delegated_wire(
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    parent_digest: [u8; 32],
) -> Result<Vec<u8>, ProgramError> {
    let delegated = Box::new(
        request
            .delegated_custody_request(parent_digest, accounts.custody_authority.key.to_bytes())
            .map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?
            .encode()
            .map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?,
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
            .map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?
            .to_bytes()
            .map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?,
    );
    Ok(plain.to_vec())
}

#[inline(never)]
fn authenticate_custody_frame(
    program_id: &Pubkey,
    accounts: ConservationAccounts<'_, '_>,
    request: ClaimsConservationRequestV1,
    custody: &CustodyRequestV1,
    request_bytes: &[u8],
) -> Result<(), ProgramError> {
    let caller = CallerAuthoritySeedsV1::new(
        ContentId::new(custody.release_set).map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?,
        custody.market,
        ExecutionRoleV1::Claims,
        custody.context,
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?;
    let replay = CustodyReplaySeedsV1::from_request(*custody);
    let authority = CustodyAuthoritySeedsV1::from_request(*custody);
    // The vault's seeds come from the contract's direction-free helper, NOT
    // from the request's source side: on a split the source is the External
    // side and its "vault" seeds derive nothing this frame holds.
    let vault = request.hoard_vault_seeds();
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
        return Err(ClaimsConservationSbfErrorV1::Identity.into());
    }
    Ok(())
}

/// The two token accounts Custody's Transfer frame names, IN ITS ORDER.
///
/// Coordinate 10 is `TransferSource` and 11 is `TransferDestination`
/// (`dclutch-custody`'s `frame_spec_v1`), and which of the vault and the
/// actor's own account stands at each depends on the DIRECTION. The order is
/// taken from the request Custody itself will decode, so the frame and the
/// wire cannot disagree.
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
    Err(ClaimsConservationSbfErrorV1::Identity.into())
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
        ContentId::new(custody.release_set).map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?,
        custody.market,
        ExecutionRoleV1::Claims,
        custody.context,
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?;
    let bump = [Pubkey::find_program_address(&caller.as_slices(), program_id).1];
    let [domain, release, market, role, context, digest] = caller.as_slices();
    // The runtime surfaces the child's own code when a CPI refuses, so this
    // `map_err` names the one cause that is this program's: a frame Custody
    // never got to decode. Custody's refusals reach the reader as Custody's.
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
    .map_err(|_| ClaimsConservationSbfErrorV1::CustodyWire)?;
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

    /// The sub-band is contiguous from `0x5300` and the two escrow codes stay
    /// the shared ones.
    #[test]
    fn the_sub_band_is_claims_0x5300_and_the_escrow_codes_are_shared() {
        assert_eq!(
            ClaimsConservationSbfErrorV1::Instruction as u32,
            dclutch_refusal_registry::CLAIMS_REFUSAL_BASE + 0x300,
        );
        assert_eq!(ClaimsConservationSbfErrorV1::ALL.len(), 13);
        assert_ne!(
            ClaimsSbfError::FailureEscrow as u32,
            ClaimsConservationSbfErrorV1::Identity as u32,
        );
    }

    /// A SPLIT THAT MOVES NO COLLATERAL REFUSES, in the contract this route
    /// dispatches, before any account is touched.
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
        request(ClaimsConservationDirectionV1::Split)
            .validate()
            .expect("an exact split is admitted");
    }

    /// WHICH TOKEN ACCOUNT CUSTODY DEBITS depends on the direction, and this
    /// route may not write the pair down.
    #[test]
    fn a_split_debits_the_actor_and_a_merge_debits_the_hoard() {
        let digest = id(0x9d);
        let split = request(ClaimsConservationDirectionV1::Split)
            .custody_request(digest)
            .expect("split custody request");
        assert_eq!(split.source, id(8));
        assert_eq!(split.destination, id(9));
        let merge = request(ClaimsConservationDirectionV1::Merge)
            .custody_request(digest)
            .expect("merge custody request");
        assert_eq!(merge.source, id(9));
        assert_eq!(merge.destination, id(8));
    }

    /// THE VAULT'S SEEDS ARE DIRECTION-FREE. The first draft derived them from
    /// the Custody request's SOURCE side, which on a split is the actor's
    /// External account: every split would have refused `Identity` at the
    /// custody frame. The contract's helper names the HoardPrincipal
    /// compartment under the Market's own namespace either way.
    #[test]
    fn the_vault_seeds_do_not_depend_on_the_direction() {
        let split = request(ClaimsConservationDirectionV1::Split);
        let merge = request(ClaimsConservationDirectionV1::Merge);
        assert_eq!(split.hoard_vault_seeds(), merge.hoard_vault_seeds());
        let digest = id(0x9d);
        let custody = split.custody_request(digest).expect("split custody");
        // The old derivation, for the record: the source side of a split is
        // External with a zero vault context, and derives nothing this frame
        // holds.
        let old = dclutch_custody::CustodyVaultSeedsV1::from_request(custody, true);
        assert_ne!(old, split.hoard_vault_seeds());
        let new = dclutch_custody::CustodyVaultSeedsV1::from_request(custody, false);
        assert_eq!(new, split.hoard_vault_seeds());
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

    /// The frame is exactly the twenty-one coordinates the constants name, in
    /// order, with no gap and no duplicate.
    #[test]
    fn the_frame_constants_tile_the_account_count() {
        let coordinates = [
            OWNER,
            AGGREGATE,
            POSITION,
            ESCROW_POSITION,
            CORE_MARKET,
            BASIS_RECORD,
            CACHE,
            REGISTRY,
            CLAIMS_PROGRAM,
            CLAIMS_PROGRAMDATA,
            CORE_PROGRAM,
            CUSTODY_CALLER_AUTHORITY,
            CUSTODY_PROGRAM,
            CUSTODY_REPLAY,
            HOARD_VAULT,
            EXTERNAL_COLLATERAL,
            COLLATERAL_MINT,
            TOKEN_PROGRAM,
            CUSTODY_AUTHORITY,
            REALM_RECORD,
            REALM_STAGING,
        ];
        assert_eq!(coordinates.len(), CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1);
        for (index, coordinate) in coordinates.iter().enumerate() {
            assert_eq!(*coordinate, index);
        }
    }
}
