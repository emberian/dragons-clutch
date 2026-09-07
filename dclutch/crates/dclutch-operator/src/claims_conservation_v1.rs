//! Build one conservative complete-set act -- a split or a merge -- from the
//! chain's own bytes.
//!
//! `dclutch-claims::conservation` owns the arithmetic and the shape of the
//! request; `programs/dclutch-claims-sbf/src/claims_conservation_v1.rs` is the
//! route that executes it. This is the host half between the two: it reads the
//! accounts the route will authenticate, DERIVES every coordinate the request
//! carries, and hands back the exact instruction pair the actor signs.
//!
//! # What the caller may state, and what it may not
//!
//! A caller states the Market, the actor, the direction and the quantity.
//! Everything else is read off authenticated bytes: the release set, the
//! Custody namespace, the Realm, the generation and the width off the LBV2
//! aggregate; the phase, the Product record digest and the principal cap off
//! Core's own state; the basis scale and whether the Market refunds off the
//! linked basis record, whose semantic identity must reproduce the aggregate's
//! `basis_id`; the cursor off the Claims-role Custody replay; the two token
//! balances off the token accounts themselves. The poststate is COMPUTED from
//! the direction and `quantity * basis_scale`, never taken from the caller,
//! because a caller that could hand this builder a poststate would be handing
//! it the answer the route's `Balances` and `Receipt` conjuncts exist to check.
//!
//! # The first draft's two domains
//!
//! The previous planner derived the aggregate under `ClaimsAggregateSeedsV1`
//! (`dclutch:claims-aggregate:v1`, the economic slice's) and the Position under
//! `ProtocolPositionSeedsV2` (LBV2's) -- CLAIMS-18's "the operator's addresses
//! in two domains". Both are LBV2 now: the aggregate is at
//! `LiabilityBasisMarketSeedsV2` under the Claims program, which is what
//! `founding_v5` created and what the route reproduces from the aggregate's
//! own recorded bump.
//!
//! # The split's second instruction
//!
//! A split debits the actor's own token account through Custody's delegated
//! wire, so the actor must approve exactly `collateral_atoms` to the Custody
//! transfer authority in the same transaction. The plan carries that
//! `ApproveChecked` FIRST, built by the tree's one Token author
//! (`dclutch_custody::token_svm::instruction::approve_checked`); a merge
//! carries no approval.

use dclutch_claims::complete_set_v1::{held_complete_sets_v1, principal_v1};
use dclutch_claims::conservation::frame_v1::{
    AGGREGATE, BASIS_RECORD, CACHE, CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1, CLAIMS_PROGRAM,
    CLAIMS_PROGRAMDATA, COLLATERAL_MINT, CORE_MARKET, CORE_PROGRAM, CUSTODY_AUTHORITY,
    CUSTODY_CALLER_AUTHORITY, CUSTODY_PROGRAM, CUSTODY_REPLAY, ESCROW_POSITION,
    EXTERNAL_COLLATERAL, HOARD_VAULT, OWNER, POSITION, REALM_RECORD, REALM_STAGING, REGISTRY,
    TOKEN_PROGRAM,
};
use dclutch_claims::conservation::{
    CLAIMS_CONSERVATION_REQUEST_BYTES_V1, ClaimsConservationDirectionV1,
    ClaimsConservationRequestV1, Error as ConservationError, collateral_atoms_v1,
};
use dclutch_claims::liability_basis_state_v2::{
    LiabilityBasisMarketSeedsV2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_claims::protocol_position_v2::{ProtocolPositionSeedsV2, failure_escrow_v1};
use dclutch_core_contract::ContentId;
use dclutch_custody::token_svm::instruction::approve_checked;
use dclutch_custody::token_svm::{Mint, TokenAccount};
use dclutch_custody::{CallerRoleV1, CustodyAuthoritySeedsV1, CustodyReplayV1};
use dclutch_market::{CoreState, MarketAdmissionV1, Phase};
use dclutch_product::payoff::runtime_v3::{ProductBasisV3, semantic_basis_id_v3};
use dclutch_registry::ACTIVATION_PDA_DOMAIN_V1;
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_source::MarketPrincipalCapSetsV1;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

/// Stable refusal from conservation planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsConservationOperatorErrorV1 {
    /// A supplied coordinate was zero or self-aliased.
    Identity,
    /// The Claims aggregate did not decode, is not at its derived address, or
    /// names another Market.
    Aggregate,
    /// The actor's Position did not decode or does not join the aggregate.
    Position,
    /// The Core Market did not decode or is not owned by the plan's Core.
    Core,
    /// The Core Market is not Open. The route refuses this by name too.
    Phase,
    /// The linked basis record does not reproduce the aggregate's `basis_id`,
    /// or its width is not the aggregate's.
    Basis,
    /// The Market refunds and no seated escrow Position was supplied, or the
    /// escrow supplied is not the derived one.
    Escrow,
    /// The Claims-role Custody replay did not decode or is another cursor.
    Replay,
    /// A token account did not parse, or is not the mint's under the owner it
    /// must have.
    Token,
    /// A merge asked for more complete sets than the Position holds.
    Holding,
    /// The vault does not back the outstanding supply at the basis scale.
    Backing,
    /// A split would grow outstanding principal past the Market's carried
    /// manipulation-capacity cap. The route refuses this by name too.
    PrincipalCapacity,
    /// The conservation contract refused the assembled request.
    Contract(ConservationError),
    /// `dclutch_claims` refused; the cause is its own.
    Claims(dclutch_claims::liability_basis_state_v2::LiabilityBasisStateErrorV2),
}

/// The programs one act addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsConservationProgramsV1 {
    /// Release-selected Claims program.
    pub claims: Pubkey,
    /// Current Claims ProgramData.
    pub claims_programdata: Pubkey,
    /// Release-selected Custody program.
    pub custody: Pubkey,
    /// Release-selected Core program.
    pub core: Pubkey,
    /// Immutable Registry program.
    pub registry: Pubkey,
}

/// The accounts one act reads, as the chain returned them at one observation.
#[derive(Clone, Copy, Debug)]
pub struct ClaimsConservationObservedV1<'a> {
    /// Canonical Core Market address.
    pub market: Pubkey,
    /// Core Market state bytes.
    pub core_state: &'a [u8],
    /// The LBV2 aggregate's bytes, at its derived address.
    pub aggregate: &'a [u8],
    /// The actor's Position bytes, at its derived address.
    pub position: &'a [u8],
    /// The Market's derived failure escrow Position bytes, when it exists.
    pub escrow_position: Option<&'a [u8]>,
    /// The finalized linked basis record's raw address and bytes. The address
    /// is carried by the founding producer (`linked_liability_basis_record` in
    /// the founding evidence) and is not re-derived here: the route hashes the
    /// bytes and joins the semantic identity, so the frame need only name
    /// where those bytes are.
    pub basis_record: (Pubkey, &'a [u8]),
    /// The Claims-role Custody replay's bytes.
    pub custody_replay: &'a [u8],
    /// The HoardPrincipal vault's token account bytes.
    pub hoard_vault: &'a [u8],
    /// The actor's collateral token account and its bytes.
    pub external_collateral: (Pubkey, &'a [u8]),
    /// The Realm's collateral mint bytes.
    pub collateral_mint: (Pubkey, &'a [u8]),
    /// The Realm's token program.
    pub token_program: Pubkey,
    /// The finalized Realm record's raw address and vacant staging cursor.
    pub realm_raw: Pubkey,
    /// See `realm_raw`.
    pub realm_staging: Pubkey,
}

/// What the actor asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsConservationActV1 {
    /// Split or merge.
    pub direction: ClaimsConservationDirectionV1,
    /// The Position owner, who signs.
    pub owner: Pubkey,
    /// Exact complete sets created or destroyed.
    pub quantity: u64,
}

/// One validated conservation act, every address it addresses, and the exact
/// instruction pair the actor signs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimsConservationPlanV1 {
    /// The validated request.
    pub request: ClaimsConservationRequestV1,
    /// Its exact canonical bytes.
    pub bytes: [u8; CLAIMS_CONSERVATION_REQUEST_BYTES_V1],
    /// The actor's `ApproveChecked` of exactly `collateral_atoms` to the
    /// Custody transfer authority; present on a split, absent on a merge.
    pub approve: Option<Instruction>,
    /// The conservation instruction over the route's exact frame.
    pub instruction: Instruction,
    /// Derived Claims aggregate.
    pub aggregate: Pubkey,
    /// Derived actor Position.
    pub position: Pubkey,
    /// Derived failure escrow Position (in the frame either way).
    pub escrow_position: Pubkey,
    /// Derived HoardPrincipal vault.
    pub hoard_vault: Pubkey,
    /// Derived Claims-role Custody replay.
    pub custody_replay: Pubkey,
    /// Derived Custody transfer authority.
    pub custody_authority: Pubkey,
    /// The Claims caller-authority PDA this act signs Custody with.
    pub custody_caller_authority: Pubkey,
    /// Exact collateral atoms the act moves.
    pub collateral_atoms: u64,
    /// Whether the Market refunds on failure, read off the record.
    pub refunds_on_failure: bool,
    /// Complete sets the actor can merge after this act, if it lands.
    pub held_complete_sets_after: u64,
}

/// Plan one conservative complete-set act from observed bytes.
///
/// # Errors
///
/// Refuses on any coordinate the route would refuse, by the same name, so an
/// operator learns at plan time what the chain would tell them at execution.
pub fn plan_claims_conservation_v1(
    programs: ClaimsConservationProgramsV1,
    observed: ClaimsConservationObservedV1<'_>,
    act: ClaimsConservationActV1,
) -> Result<ClaimsConservationPlanV1, ClaimsConservationOperatorErrorV1> {
    if [
        programs.claims,
        programs.claims_programdata,
        programs.custody,
        programs.core,
        programs.registry,
        observed.market,
        act.owner,
        observed.external_collateral.0,
        observed.collateral_mint.0,
        observed.basis_record.0,
        observed.token_program,
        observed.realm_raw,
        observed.realm_staging,
    ]
    .iter()
    .any(|key| key.to_bytes() == [0; 32])
    {
        return Err(ClaimsConservationOperatorErrorV1::Identity);
    }

    // ---- the aggregate, at the LBV2 address founding created -------------
    let aggregate_key = Pubkey::find_program_address(
        &LiabilityBasisMarketSeedsV2::new(observed.market.to_bytes())
            .map_err(ClaimsConservationOperatorErrorV1::Claims)?
            .as_slices(),
        &programs.claims,
    )
    .0;
    let market = LiabilityBasisMarketViewV2::decode(observed.aggregate)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Aggregate)?;
    if market.logical_market != observed.market.to_bytes()
        || market.registry_program != programs.registry.to_bytes()
    {
        return Err(ClaimsConservationOperatorErrorV1::Aggregate);
    }

    // ---- Core: phase by name, the Product record digest, the cap ---------
    let core = CoreState::decode(observed.core_state)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Core)?;
    if core.identity.market_id.to_bytes() != observed.market.to_bytes()
        || core.identity.selected_release_set.to_bytes() != market.release_set
        || core.identity.generation != market.generation
    {
        return Err(ClaimsConservationOperatorErrorV1::Core);
    }
    if !MarketAdmissionV1::phases(&[Phase::Open]).admits_phase(core.phase) {
        return Err(ClaimsConservationOperatorErrorV1::Phase);
    }

    // ---- the record: one account proves it -------------------------------
    let semantic = semantic_basis_id_v3(observed.basis_record.1)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Basis)?;
    let basis = ProductBasisV3::decode(observed.basis_record.1)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Basis)?;
    if semantic != market.basis_id || basis.basis_width() != market.claim_count {
        return Err(ClaimsConservationOperatorErrorV1::Basis);
    }
    let basis_scale = basis.payout_scale();
    let refunds_on_failure = basis.refunds_on_failure();
    let linked_basis_record_digest = hash(observed.basis_record.1).to_bytes();

    // ---- the actor's Position --------------------------------------------
    let position_key = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate_key.to_bytes(), act.owner.to_bytes())
            .map_err(|_| ClaimsConservationOperatorErrorV1::Identity)?
            .as_slices(),
        &programs.claims,
    )
    .0;
    let position = LiabilityBasisPositionViewV2::decode(observed.position)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Position)?;
    if position.market_account != aggregate_key.to_bytes()
        || position.owner != act.owner.to_bytes()
        || position.basis_id != market.basis_id
        || position.claim_count != market.claim_count
    {
        return Err(ClaimsConservationOperatorErrorV1::Position);
    }
    let held = held_complete_sets_v1(observed.position, refunds_on_failure)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Position)?;
    let split = matches!(act.direction, ClaimsConservationDirectionV1::Split);
    if !split && act.quantity > held {
        return Err(ClaimsConservationOperatorErrorV1::Holding);
    }
    let held_complete_sets_after = if split {
        held.checked_add(act.quantity)
            .ok_or(ClaimsConservationOperatorErrorV1::Holding)?
    } else {
        held - act.quantity
    };

    // ---- the escrow, derived and (on a refunding Market) seated ----------
    let escrow = failure_escrow_v1(
        programs.claims,
        observed.market.to_bytes(),
        aggregate_key,
        market.claim_count,
    )
    .map_err(|_| ClaimsConservationOperatorErrorV1::Escrow)?;
    if refunds_on_failure {
        let bytes = observed
            .escrow_position
            .ok_or(ClaimsConservationOperatorErrorV1::Escrow)?;
        let view = LiabilityBasisPositionViewV2::decode(bytes)
            .map_err(|_| ClaimsConservationOperatorErrorV1::Escrow)?;
        let seated = view
            .balance(bytes, escrow.failure_selector)
            .map_err(|_| ClaimsConservationOperatorErrorV1::Escrow)?;
        let supply = market
            .supply(observed.aggregate, escrow.failure_selector)
            .map_err(|_| ClaimsConservationOperatorErrorV1::Escrow)?;
        if view.owner != escrow.owner.to_bytes() || supply == 0 || seated != supply {
            return Err(ClaimsConservationOperatorErrorV1::Escrow);
        }
    }

    // ---- Custody: the cursor, the vault, the authority --------------------
    let replay = CustodyReplayV1::decode(observed.custody_replay)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Replay)?;
    if replay.caller_role != CallerRoleV1::Claims
        || replay.market != observed.market.to_bytes()
        || replay.release_set != market.release_set
        || replay.context != market.custody_context
        || replay.caller_program != programs.claims.to_bytes()
    {
        return Err(ClaimsConservationOperatorErrorV1::Replay);
    }
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(observed.market.to_bytes(), market.release_set).as_slices(),
        &programs.custody,
    )
    .0;
    let mint = Mint::parse(observed.collateral_mint.1)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Token)?;
    let vault = TokenAccount::parse_base_or_immutable_owner(observed.hoard_vault)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Token)?;
    let external = TokenAccount::parse_base_or_immutable_owner(observed.external_collateral.1)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Token)?;
    if vault.mint != observed.collateral_mint.0.to_bytes()
        || vault.owner != custody_authority.to_bytes()
        || external.mint != observed.collateral_mint.0.to_bytes()
        || external.owner != act.owner.to_bytes()
    {
        return Err(ClaimsConservationOperatorErrorV1::Token);
    }
    let principal = principal_v1(observed.aggregate, vault.amount, basis_scale)
        .map_err(|_| ClaimsConservationOperatorErrorV1::Backing)?;

    // ---- the request, its poststate derived ------------------------------
    let atoms = collateral_atoms_v1(act.quantity, basis_scale)
        .map_err(ClaimsConservationOperatorErrorV1::Contract)?;
    let (post_external, post_hoard) =
        if split {
            (
                external.amount.checked_sub(atoms).ok_or(
                    ClaimsConservationOperatorErrorV1::Contract(
                        ConservationError::ExternalBalanceMismatch,
                    ),
                )?,
                vault.amount.checked_add(atoms).ok_or(
                    ClaimsConservationOperatorErrorV1::Contract(
                        ConservationError::HoardBalanceMismatch,
                    ),
                )?,
            )
        } else {
            (
                external.amount.checked_add(atoms).ok_or(
                    ClaimsConservationOperatorErrorV1::Contract(
                        ConservationError::ExternalBalanceMismatch,
                    ),
                )?,
                vault.amount.checked_sub(atoms).ok_or(
                    ClaimsConservationOperatorErrorV1::Contract(
                        ConservationError::HoardBalanceMismatch,
                    ),
                )?,
            )
        };
    let request = ClaimsConservationRequestV1 {
        direction: act.direction,
        realm: market.realm_id,
        market: observed.market.to_bytes(),
        release_set: market.release_set,
        custody_context: market.custody_context,
        aggregate: aggregate_key.to_bytes(),
        position: position_key.to_bytes(),
        owner: act.owner.to_bytes(),
        external_collateral: observed.external_collateral.0.to_bytes(),
        hoard_vault: [0; 32],
        mint: observed.collateral_mint.0.to_bytes(),
        token_program: observed.token_program.to_bytes(),
        claims_program: programs.claims.to_bytes(),
        product_record_digest: core.identity.product_record.to_bytes(),
        linked_basis_record_digest,
        semantic_basis_id: market.basis_id,
        generation: market.generation,
        quantity: act.quantity,
        basis_scale,
        collateral_atoms: atoms,
        expected_market_revision: market.revision,
        expected_position_revision: position.revision,
        expected_custody_revision: replay.next_revision,
        pre_external_amount: external.amount,
        post_external_amount: post_external,
        pre_hoard_amount: vault.amount,
        post_hoard_amount: post_hoard,
        claim_count: market.claim_count,
    };
    // The vault is derived through the contract's own direction-free seed
    // helper and only then written into the request, so the field and the
    // derivation cannot disagree.
    let hoard_vault =
        Pubkey::find_program_address(&request.hoard_vault_seeds().as_slices(), &programs.custody).0;
    let request = ClaimsConservationRequestV1 {
        hoard_vault: hoard_vault.to_bytes(),
        ..request
    };
    request
        .validate()
        .map_err(ClaimsConservationOperatorErrorV1::Contract)?;
    // Core's carried cap, read here because the route reads it: a split that
    // would grow outstanding principal past it is refused at plan time by the
    // same name the chain would refuse it under, which is the whole promise
    // this function makes.
    request
        .admit_capacity(
            principal.outstanding_sets,
            MarketPrincipalCapSetsV1::read(core.principal_cap_sets).to_sets(),
        )
        .map_err(|_| ClaimsConservationOperatorErrorV1::PrincipalCapacity)?;
    let bytes = request
        .to_bytes()
        .map_err(ClaimsConservationOperatorErrorV1::Contract)?;
    let parent_digest = hash(&bytes).to_bytes();
    let custody_replay = Pubkey::find_program_address(
        &request.custody_replay_seeds().as_slices(),
        &programs.custody,
    )
    .0;

    // ---- the Custody wire the route derives, and the PDA it signs with ----
    let custody = request
        .custody_request(parent_digest)
        .map_err(ClaimsConservationOperatorErrorV1::Contract)?;
    let wire_digest = if split {
        hash(
            &request
                .delegated_custody_request(parent_digest, custody_authority.to_bytes())
                .map_err(ClaimsConservationOperatorErrorV1::Contract)?
                .encode()
                .map_err(|_| {
                    ClaimsConservationOperatorErrorV1::Contract(ConservationError::CustodyShape)
                })?,
        )
        .to_bytes()
    } else {
        hash(&custody.to_bytes().map_err(|_| {
            ClaimsConservationOperatorErrorV1::Contract(ConservationError::CustodyShape)
        })?)
        .to_bytes()
    };
    let custody_caller_authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(custody.release_set)
                .map_err(|_| ClaimsConservationOperatorErrorV1::Identity)?,
            custody.market,
            ExecutionRoleV1::Claims,
            custody.context,
            wire_digest,
        )
        .map_err(|_| ClaimsConservationOperatorErrorV1::Identity)?
        .as_slices(),
        &programs.claims,
    )
    .0;
    let activation_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &market.release_set],
        &programs.registry,
    )
    .0;

    // ---- the frame, in the route's own coordinate order ------------------
    // Every coordinate is seated by its own named constant, so a frame that
    // grows a coordinate fails to compile here rather than shipping a hole.
    let mut accounts = vec![
        AccountMeta::new_readonly(Pubkey::default(), false);
        CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1
    ];
    let mut seat = |coordinate: usize, meta: AccountMeta| {
        *accounts
            .get_mut(coordinate)
            .expect("the frame's coordinates tile its own account count") = meta;
    };
    seat(OWNER, AccountMeta::new(act.owner, true));
    seat(AGGREGATE, AccountMeta::new(aggregate_key, false));
    seat(POSITION, AccountMeta::new(position_key, false));
    seat(ESCROW_POSITION, AccountMeta::new(escrow.position, false));
    seat(
        CORE_MARKET,
        AccountMeta::new_readonly(observed.market, false),
    );
    seat(
        BASIS_RECORD,
        AccountMeta::new_readonly(observed.basis_record.0, false),
    );
    seat(CACHE, AccountMeta::new_readonly(activation_cache, false));
    seat(
        REGISTRY,
        AccountMeta::new_readonly(programs.registry, false),
    );
    seat(
        CLAIMS_PROGRAM,
        AccountMeta::new_readonly(programs.claims, false),
    );
    seat(
        CLAIMS_PROGRAMDATA,
        AccountMeta::new_readonly(programs.claims_programdata, false),
    );
    seat(
        CORE_PROGRAM,
        AccountMeta::new_readonly(programs.core, false),
    );
    seat(
        CUSTODY_CALLER_AUTHORITY,
        AccountMeta::new_readonly(custody_caller_authority, false),
    );
    seat(
        CUSTODY_PROGRAM,
        AccountMeta::new_readonly(programs.custody, false),
    );
    seat(CUSTODY_REPLAY, AccountMeta::new(custody_replay, false));
    seat(HOARD_VAULT, AccountMeta::new(hoard_vault, false));
    seat(
        EXTERNAL_COLLATERAL,
        AccountMeta::new(observed.external_collateral.0, false),
    );
    seat(
        COLLATERAL_MINT,
        AccountMeta::new_readonly(observed.collateral_mint.0, false),
    );
    seat(
        TOKEN_PROGRAM,
        AccountMeta::new_readonly(observed.token_program, false),
    );
    seat(
        CUSTODY_AUTHORITY,
        AccountMeta::new_readonly(custody_authority, false),
    );
    seat(
        REALM_RECORD,
        AccountMeta::new_readonly(observed.realm_raw, false),
    );
    seat(
        REALM_STAGING,
        AccountMeta::new_readonly(observed.realm_staging, false),
    );
    drop(seat);

    let approve = if split {
        let spec = approve_checked(
            observed.token_program.to_bytes(),
            observed.external_collateral.0.to_bytes(),
            observed.collateral_mint.0.to_bytes(),
            custody_authority.to_bytes(),
            act.owner.to_bytes(),
            atoms,
            mint.decimals,
        )
        .map_err(|_| ClaimsConservationOperatorErrorV1::Token)?;
        Some(Instruction {
            program_id: Pubkey::new_from_array(*spec.program_id()),
            accounts: spec
                .accounts()
                .iter()
                .map(|account| AccountMeta {
                    pubkey: Pubkey::new_from_array(*account.address()),
                    is_signer: account.is_signer(),
                    is_writable: account.is_writable(),
                })
                .collect(),
            data: spec.data().to_vec(),
        })
    } else {
        None
    };

    Ok(ClaimsConservationPlanV1 {
        request,
        bytes,
        approve,
        instruction: Instruction {
            program_id: programs.claims,
            accounts,
            data: bytes.to_vec(),
        },
        aggregate: aggregate_key,
        position: position_key,
        escrow_position: escrow.position,
        hoard_vault,
        custody_replay,
        custody_authority,
        custody_caller_authority,
        collateral_atoms: atoms,
        refunds_on_failure,
        held_complete_sets_after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_claims::liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
        encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
        liability_basis_vector_width_v2,
    };
    use dclutch_custody::CompartmentV1;
    use dclutch_custody::token_svm::state::{MintLayoutV1, TokenAccountLayoutV1};
    use dclutch_custody::token_svm::{LEGACY_TOKEN_PROGRAM_ID, MINT_BYTES};
    use dclutch_market::{Identity, MarketIdentity, Readiness, StateBumpsV1};
    use dclutch_product::payoff::runtime_v3::{
        BASIS_PAYOUT_SCALE_OFFSET_V3, BasisInputV3, BasisKindV3, basis_record_bytes_v3,
        compile_basis_v3,
    };

    /// Runtime complete-set width: three ordinary regions and one failure
    /// coordinate, which is cohort-13's own shape.
    const WIDTH: u32 = 4;
    /// The scale a Market founded to refund carries, `basis_width - 1`.
    const REFUNDING_SCALE: u64 = 3;
    /// Complete sets one act creates or destroys.
    const QUANTITY: u64 = 5;
    /// Outstanding complete sets the aggregate reports at every coordinate.
    const SUPPLY: u64 = 10;
    /// Complete sets the actor's Position holds before the act.
    const HELD: u64 = 8;
    const VAULT_ATOMS: u64 = 1_000;
    const EXTERNAL_ATOMS: u64 = 1_000;
    const GENERATION: u64 = 9;
    const RELEASE_SET: [u8; 32] = [0x21; 32];
    const CUSTODY_CONTEXT: [u8; 32] = [0x22; 32];
    const REALM_ID: [u8; 32] = [0x23; 32];

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn id(value: u8) -> Identity {
        Identity::new([value; 32]).expect("identity")
    }

    /// The frame's coordinates are the route's, read off the route.
    #[test]
    fn the_frame_constants_are_the_routes_own() {
        assert_eq!(CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1, 21);
        assert_eq!(OWNER, 0);
        assert_eq!(REALM_STAGING, 20);
    }

    /// A zero program or actor refuses before any byte is decoded.
    #[test]
    fn a_zero_coordinate_refuses_by_name() {
        let programs = ClaimsConservationProgramsV1 {
            claims: Pubkey::new_unique(),
            claims_programdata: Pubkey::new_unique(),
            custody: Pubkey::new_unique(),
            core: Pubkey::new_unique(),
            registry: Pubkey::new_unique(),
        };
        let observed = ClaimsConservationObservedV1 {
            market: Pubkey::default(),
            core_state: &[],
            aggregate: &[],
            position: &[],
            escrow_position: None,
            basis_record: (Pubkey::new_unique(), &[]),
            custody_replay: &[],
            hoard_vault: &[],
            external_collateral: (Pubkey::new_unique(), &[]),
            collateral_mint: (Pubkey::new_unique(), &[]),
            token_program: Pubkey::new_unique(),
            realm_raw: Pubkey::new_unique(),
            realm_staging: Pubkey::new_unique(),
        };
        assert_eq!(
            plan_claims_conservation_v1(
                programs,
                observed,
                ClaimsConservationActV1 {
                    direction: ClaimsConservationDirectionV1::Split,
                    owner: Pubkey::new_unique(),
                    quantity: 1,
                },
            ),
            Err(ClaimsConservationOperatorErrorV1::Identity),
        );
    }

    /// What one observation of a Market may differ in.
    #[derive(Clone, Copy)]
    struct Scenario {
        /// Atoms per complete set, as the linked basis record carries it.
        basis_scale: u64,
        /// Outstanding sets at every aggregate coordinate.
        supply: u64,
        /// Complete sets the actor's Position holds.
        held: u64,
        /// The actor's own collateral balance.
        external_atoms: u64,
        /// The HoardPrincipal vault's balance.
        vault_atoms: u64,
        /// Core's carried manipulation-capacity cap, in complete sets.
        /// `u64::MAX` is the explicit unbounded sentinel.
        principal_cap_sets: u64,
        /// Overwrite the record's payout scale with zero after compiling it.
        /// The compiler will not write such a record, so this is the only way
        /// one reaches the planner at all.
        forge_a_zero_scale: bool,
    }

    impl Scenario {
        /// A refunding Market: the scale is `basis_width - 1`, so a set and its
        /// collateral are different numbers and the escrow is seated.
        fn refunding() -> Self {
            Self {
                basis_scale: REFUNDING_SCALE,
                supply: SUPPLY,
                held: HELD,
                external_atoms: EXTERNAL_ATOMS,
                vault_atoms: VAULT_ATOMS,
                principal_cap_sets: u64::MAX,
                forge_a_zero_scale: false,
            }
        }
    }

    /// One observation's bytes, at addresses the planner will re-derive.
    struct Fixture {
        programs: ClaimsConservationProgramsV1,
        owner: Pubkey,
        market: Pubkey,
        basis_record_key: Pubkey,
        external_collateral_key: Pubkey,
        collateral_mint_key: Pubkey,
        token_program: Pubkey,
        realm_raw: Pubkey,
        realm_staging: Pubkey,
        core_state: Vec<u8>,
        aggregate: Vec<u8>,
        position: Vec<u8>,
        escrow_position: Vec<u8>,
        basis_record: Vec<u8>,
        custody_replay: Vec<u8>,
        hoard_vault: Vec<u8>,
        external_collateral: Vec<u8>,
        collateral_mint: Vec<u8>,
    }

    fn basis_record_bytes(scenario: Scenario) -> Vec<u8> {
        let width = basis_record_bytes_v3(
            BasisKindV3::CategoricalQ1,
            usize::try_from(WIDTH).expect("width"),
            0,
            0,
        )
        .expect("record width");
        let mut bytes = vec![0_u8; width];
        compile_basis_v3(
            BasisInputV3 {
                kind: BasisKindV3::CategoricalQ1,
                product_id: [0x31; 32],
                result_domain_id: [0x32; 32],
                coordinate_domain_id: [0x33; 32],
                result_unit_id: [0x34; 32],
                evaluator_release_id: [0x35; 32],
                basis_width: WIDTH,
                payout_scale: scenario.basis_scale,
                knot_denominator: 1,
                knots: &[],
                terms: &[],
                failure_payouts: &[],
                price_gate_certificate_digest: [0; 32],
            },
            &mut bytes,
        )
        .expect("basis record");
        if scenario.forge_a_zero_scale {
            bytes
                .get_mut(BASIS_PAYOUT_SCALE_OFFSET_V3..BASIS_PAYOUT_SCALE_OFFSET_V3 + 8)
                .expect("payout scale field")
                .fill(0);
        }
        bytes
    }

    fn aggregate_bytes(
        basis_id: [u8; 32],
        market: Pubkey,
        registry: Pubkey,
        supply: u64,
    ) -> Vec<u8> {
        let mut bytes =
            vec![
                0_u8;
                liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, WIDTH)
                    .expect("aggregate width")
            ];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 2,
                logical_market: market.to_bytes(),
                release_set: RELEASE_SET,
                registry_program: registry.to_bytes(),
                product_instance_id: [0x24; 32],
                basis_id,
                realm_id: REALM_ID,
                custody_context: CUSTODY_CONTEXT,
                generation: GENERATION,
            },
            &vec![supply; usize::try_from(WIDTH).expect("width")],
            &mut bytes,
        )
        .expect("aggregate");
        bytes
    }

    fn position_bytes(
        aggregate: Pubkey,
        owner: Pubkey,
        basis_id: [u8; 32],
        revision: u64,
        balances: &[u64],
    ) -> Vec<u8> {
        let mut bytes =
            vec![
                0_u8;
                liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, WIDTH)
                    .expect("position width")
            ];
        encode_liability_basis_position_into_v2(
            LiabilityBasisPositionInputV2 {
                revision,
                market_account: aggregate.to_bytes(),
                owner: owner.to_bytes(),
                basis_id,
            },
            balances,
            &mut bytes,
        )
        .expect("position");
        bytes
    }

    fn token_account_bytes(mint: Pubkey, owner: Pubkey, amount: u64) -> Vec<u8> {
        let mut bytes = TokenAccount::initialized_base_bytes(mint.to_bytes(), owner.to_bytes())
            .expect("token account")
            .to_vec();
        bytes
            .get_mut(TokenAccountLayoutV1::AMOUNT..TokenAccountLayoutV1::AMOUNT + 8)
            .expect("amount field")
            .copy_from_slice(&amount.to_le_bytes());
        bytes
    }

    fn mint_bytes(decimals: u8) -> Vec<u8> {
        let mut bytes = vec![0_u8; MINT_BYTES];
        bytes
            .get_mut(MintLayoutV1::SUPPLY..MintLayoutV1::SUPPLY + 8)
            .expect("supply field")
            .copy_from_slice(&1_000_000_u64.to_le_bytes());
        *bytes.get_mut(MintLayoutV1::DECIMALS).expect("decimals") = decimals;
        *bytes
            .get_mut(MintLayoutV1::IS_INITIALIZED)
            .expect("initialized") = 1;
        bytes
    }

    impl Fixture {
        fn new(scenario: Scenario) -> Self {
            let programs = ClaimsConservationProgramsV1 {
                claims: key(0x11),
                claims_programdata: key(0x12),
                custody: key(0x13),
                core: key(0x14),
                registry: key(0x15),
            };
            let market = key(0x16);
            let owner = key(0x17);
            let collateral_mint_key = key(0x18);

            let basis_record = basis_record_bytes(scenario);
            // A forged zero-scale record has no semantic identity, so the
            // aggregate carries a placeholder and the planner refuses on the
            // record itself rather than on the join.
            let basis_id = semantic_basis_id_v3(&basis_record).unwrap_or([0x36; 32]);

            let aggregate_key = Pubkey::find_program_address(
                &LiabilityBasisMarketSeedsV2::new(market.to_bytes())
                    .expect("aggregate seeds")
                    .as_slices(),
                &programs.claims,
            )
            .0;
            let escrow =
                failure_escrow_v1(programs.claims, market.to_bytes(), aggregate_key, WIDTH)
                    .expect("failure escrow");
            let custody_authority = Pubkey::find_program_address(
                &CustodyAuthoritySeedsV1::new(market.to_bytes(), RELEASE_SET).as_slices(),
                &programs.custody,
            )
            .0;

            let mut escrow_balances = vec![0_u64; usize::try_from(WIDTH).expect("width")];
            *escrow_balances
                .get_mut(usize::try_from(escrow.failure_selector).expect("selector"))
                .expect("failure coordinate") = scenario.supply;
            let mut holder_balances = vec![scenario.held; usize::try_from(WIDTH).expect("width")];
            *holder_balances
                .get_mut(usize::try_from(escrow.failure_selector).expect("selector"))
                .expect("failure coordinate") = 0;

            Self {
                core_state: CoreState {
                    phase: Phase::Open,
                    readiness: Readiness::Consumed,
                    terminal_winner: 0,
                    identity: MarketIdentity {
                        market_id: Identity::new(market.to_bytes()).expect("market identity"),
                        realm_id: Identity::new(REALM_ID).expect("realm identity"),
                        product_record: id(0x25),
                        product_id: id(0x26),
                        resolution_policy: id(0x27),
                        capability_manifest: id(0x28),
                        selected_release_set: Identity::new(RELEASE_SET).expect("release set"),
                        registry_program: Identity::new(programs.registry.to_bytes())
                            .expect("registry identity"),
                        generation: GENERATION,
                    },
                    outstanding_capabilities: 0,
                    principal_cap_sets: scenario.principal_cap_sets,
                    rent_beneficiary: id(0x29),
                    terminal_receipt: None,
                    bumps: StateBumpsV1::UNRECORDED,
                }
                .encode()
                .expect("core state")
                .to_vec(),
                aggregate: aggregate_bytes(basis_id, market, programs.registry, scenario.supply),
                position: position_bytes(aggregate_key, owner, basis_id, 3, &holder_balances),
                escrow_position: position_bytes(
                    aggregate_key,
                    escrow.owner,
                    basis_id,
                    1,
                    &escrow_balances,
                ),
                custody_replay: CustodyReplayV1 {
                    caller_role: CallerRoleV1::Claims,
                    release_set: RELEASE_SET,
                    market: market.to_bytes(),
                    realm: REALM_ID,
                    context: CUSTODY_CONTEXT,
                    caller_program: programs.claims.to_bytes(),
                    rent_refund: [0x2a; 32],
                    open_vault_count: 1,
                    next_revision: 4,
                    generation: GENERATION,
                    last_request_digest: [0x2b; 32],
                    last_poststate_commitment: [0x2c; 32],
                }
                .to_bytes()
                .expect("custody replay")
                .to_vec(),
                hoard_vault: token_account_bytes(
                    collateral_mint_key,
                    custody_authority,
                    scenario.vault_atoms,
                ),
                external_collateral: token_account_bytes(
                    collateral_mint_key,
                    owner,
                    scenario.external_atoms,
                ),
                collateral_mint: mint_bytes(6),
                basis_record,
                basis_record_key: key(0x19),
                external_collateral_key: key(0x1a),
                collateral_mint_key,
                token_program: Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID),
                realm_raw: key(0x1b),
                realm_staging: key(0x1c),
                programs,
                owner,
                market,
            }
        }

        fn observed(&self) -> ClaimsConservationObservedV1<'_> {
            ClaimsConservationObservedV1 {
                market: self.market,
                core_state: &self.core_state,
                aggregate: &self.aggregate,
                position: &self.position,
                escrow_position: Some(&self.escrow_position),
                basis_record: (self.basis_record_key, &self.basis_record),
                custody_replay: &self.custody_replay,
                hoard_vault: &self.hoard_vault,
                external_collateral: (self.external_collateral_key, &self.external_collateral),
                collateral_mint: (self.collateral_mint_key, &self.collateral_mint),
                token_program: self.token_program,
                realm_raw: self.realm_raw,
                realm_staging: self.realm_staging,
            }
        }

        fn plan(
            &self,
            direction: ClaimsConservationDirectionV1,
            quantity: u64,
        ) -> Result<ClaimsConservationPlanV1, ClaimsConservationOperatorErrorV1> {
            plan_claims_conservation_v1(
                self.programs,
                self.observed(),
                ClaimsConservationActV1 {
                    direction,
                    owner: self.owner,
                    quantity,
                },
            )
        }
    }

    /// The amount an `ApproveChecked` authorizes, read back off its own data.
    fn approved_amount(instruction: &Instruction) -> Option<u64> {
        let mut amount = [0_u8; 8];
        amount.copy_from_slice(instruction.data.get(1..9)?);
        Some(u64::from_le_bytes(amount))
    }

    /// The collateral a split moves is `quantity * basis_scale`, and the stated
    /// poststate is the observed prestate moved by exactly that on both sides.
    #[test]
    fn a_split_moves_quantity_times_basis_scale_and_round_trips() {
        let fixture = Fixture::new(Scenario::refunding());
        let plan = fixture
            .plan(ClaimsConservationDirectionV1::Split, QUANTITY)
            .expect("split plan");
        let atoms = QUANTITY * REFUNDING_SCALE;
        assert_eq!(plan.collateral_atoms, atoms);
        assert_eq!(plan.request.basis_scale, REFUNDING_SCALE);
        assert_eq!(plan.request.pre_external_amount, EXTERNAL_ATOMS);
        assert_eq!(plan.request.post_external_amount, EXTERNAL_ATOMS - atoms);
        assert_eq!(plan.request.pre_hoard_amount, VAULT_ATOMS);
        assert_eq!(plan.request.post_hoard_amount, VAULT_ATOMS + atoms);
        let decoded = ClaimsConservationRequestV1::decode(&plan.bytes).expect("decode");
        assert_eq!(decoded, plan.request);
        assert_eq!(decoded.validate(), Ok(()));
    }

    /// The direction is the whole difference: a merge of the same quantity on
    /// the same Market returns exactly the atoms the split took, so the round
    /// trip is the identity on both token balances.
    #[test]
    fn a_merge_returns_the_same_collateral_class_it_took() {
        let before = Fixture::new(Scenario::refunding());
        let split = before
            .plan(ClaimsConservationDirectionV1::Split, QUANTITY)
            .expect("split plan");
        let after = Fixture::new(Scenario {
            external_atoms: split.request.post_external_amount,
            vault_atoms: split.request.post_hoard_amount,
            ..Scenario::refunding()
        });
        let merge = after
            .plan(ClaimsConservationDirectionV1::Merge, QUANTITY)
            .expect("merge plan");
        assert_eq!(merge.collateral_atoms, split.collateral_atoms);
        assert_eq!(
            merge.request.post_external_amount,
            split.request.pre_external_amount
        );
        assert_eq!(
            merge.request.post_hoard_amount,
            split.request.pre_hoard_amount
        );
        assert_eq!(
            merge.request.direction.source_compartment(),
            CompartmentV1::HoardPrincipal
        );
        assert_eq!(
            merge.request.direction.destination_compartment(),
            CompartmentV1::External
        );
        // A merge spends the vault's own delegation, so it carries no approval;
        // the split's is the actor authorizing exactly what it hands over.
        assert_eq!(merge.approve, None);
        assert_eq!(
            split.approve.as_ref().and_then(approved_amount),
            Some(split.collateral_atoms)
        );
    }

    /// An act whose collateral cannot be covered is a named refusal, never a
    /// clamp to what is on hand. A split is bounded by the actor's balance; a
    /// merge is bounded twice, and the backing inequality is the tighter of the
    /// two whenever the aggregate reports the sets the actor is holding.
    #[test]
    fn an_uncoverable_act_refuses_rather_than_saturating() {
        let atoms = QUANTITY * REFUNDING_SCALE;

        let short = Fixture::new(Scenario {
            external_atoms: atoms - 1,
            ..Scenario::refunding()
        });
        assert_eq!(
            short.plan(ClaimsConservationDirectionV1::Split, QUANTITY),
            Err(ClaimsConservationOperatorErrorV1::Contract(
                ConservationError::ExternalBalanceMismatch
            )),
        );

        let drained = Fixture::new(Scenario {
            vault_atoms: atoms - 1,
            ..Scenario::refunding()
        });
        assert_eq!(
            drained.plan(ClaimsConservationDirectionV1::Merge, QUANTITY),
            Err(ClaimsConservationOperatorErrorV1::Backing),
        );

        // Where the aggregate reports fewer outstanding sets than the actor
        // holds, the backing inequality passes and the subtraction itself is
        // what refuses.
        let underreporting = Fixture::new(Scenario {
            supply: 1,
            vault_atoms: atoms - 1,
            ..Scenario::refunding()
        });
        assert_eq!(
            underreporting.plan(ClaimsConservationDirectionV1::Merge, QUANTITY),
            Err(ClaimsConservationOperatorErrorV1::Contract(
                ConservationError::HoardBalanceMismatch
            )),
        );
    }

    /// The unit-scale blindness, stated as a test rather than as a comment: at
    /// `basis_scale == 1` a set and its collateral are the same number, so a
    /// planner that confused the two would be indistinguishable here. At the
    /// refunding scale they differ, and every field must then carry its own
    /// unit -- sets in the holding, atoms on the wire and in the balances.
    #[test]
    fn a_unit_scale_hides_the_set_versus_atom_distinction_and_a_real_scale_does_not() {
        assert_eq!(REFUNDING_SCALE, u64::from(WIDTH) - 1);

        let unit = Fixture::new(Scenario {
            basis_scale: 1,
            ..Scenario::refunding()
        });
        let unit = unit
            .plan(ClaimsConservationDirectionV1::Split, QUANTITY)
            .expect("unit-scale plan");
        assert!(!unit.refunds_on_failure);
        assert_eq!(unit.collateral_atoms, unit.request.quantity);

        let scaled = Fixture::new(Scenario::refunding());
        let scaled = scaled
            .plan(ClaimsConservationDirectionV1::Split, QUANTITY)
            .expect("refunding-scale plan");
        assert!(scaled.refunds_on_failure);
        assert_ne!(scaled.collateral_atoms, scaled.request.quantity);
        assert_eq!(scaled.request.quantity, QUANTITY);
        assert_eq!(scaled.collateral_atoms, QUANTITY * REFUNDING_SCALE);
        // Sets where sets belong.
        assert_eq!(scaled.held_complete_sets_after, HELD + QUANTITY);
        // Atoms where atoms belong: the balance delta the request states, and
        // the allowance the actor signs over to Custody.
        assert_eq!(
            scaled.request.pre_external_amount - scaled.request.post_external_amount,
            QUANTITY * REFUNDING_SCALE
        );
        assert_eq!(
            scaled.approve.as_ref().and_then(approved_amount),
            Some(QUANTITY * REFUNDING_SCALE)
        );
    }

    /// Zero quantity is refused at the contract's own arithmetic boundary. A
    /// zero scale never gets that far: no record can be compiled with one, so
    /// the only zero-scale record is forged and the basis decode refuses it.
    #[test]
    fn a_zero_quantity_or_scale_is_refused() {
        let fixture = Fixture::new(Scenario::refunding());
        assert_eq!(
            fixture.plan(ClaimsConservationDirectionV1::Split, 0),
            Err(ClaimsConservationOperatorErrorV1::Contract(
                ConservationError::InvalidQuantity
            )),
        );

        let forged = Fixture::new(Scenario {
            forge_a_zero_scale: true,
            ..Scenario::refunding()
        });
        assert_eq!(
            forged.plan(ClaimsConservationDirectionV1::Split, QUANTITY),
            Err(ClaimsConservationOperatorErrorV1::Basis),
        );
    }

    /// A split past Core's carried principal cap refuses at PLAN time, by the
    /// name the chain refuses it under.
    ///
    /// The module head says this planner reads "the principal cap off Core's
    /// own state" and the function says it "refuses on any coordinate the route
    /// would refuse, by the same name". Until this test existed neither was
    /// true: `core.principal_cap_sets` was never read, so a capped Market
    /// planned splits the chain rejects at
    /// `ClaimsConservationSbfErrorV1::PrincipalCapacity`. The arms below are
    /// the cap's two REACHABLE readings, because a cap that only ever refuses
    /// is not a cap. The third, `Absent`, is unreachable through Core:
    /// `CoreState::valid_static` requires `principal_cap_sets != 0` in every
    /// phase, so a zero cap does not encode and the route's
    /// refuses-every-positive-count reading of it defends against a bent
    /// account rather than a live one.
    #[test]
    fn a_split_past_the_carried_principal_cap_refuses_at_plan_time() {
        let capped = |sets: u64| {
            Fixture::new(Scenario {
                principal_cap_sets: sets,
                ..Scenario::refunding()
            })
        };
        assert_eq!(
            capped(SUPPLY + QUANTITY - 1).plan(ClaimsConservationDirectionV1::Split, QUANTITY),
            Err(ClaimsConservationOperatorErrorV1::PrincipalCapacity),
            "a bounded cap one set short of the poststate refuses the growth",
        );
        assert!(
            capped(SUPPLY + QUANTITY)
                .plan(ClaimsConservationDirectionV1::Split, QUANTITY)
                .is_ok(),
            "and admits the split that lands exactly on it",
        );
        assert!(
            capped(u64::MAX)
                .plan(ClaimsConservationDirectionV1::Split, QUANTITY)
                .is_ok(),
            "and the explicit unbounded sentinel admits it",
        );
        assert!(
            capped(1)
                .plan(ClaimsConservationDirectionV1::Merge, QUANTITY)
                .is_ok(),
            "a merge shrinks outstanding principal, so the tightest cap a Core \
             state can carry does not bear on it",
        );
    }
}
