//! One author for the runtime frame of every General action.
//!
//! `devnet-general-session` could frame exactly one of the fifteen actions,
//! because its subject derivation read the capability root alone and its
//! runtime-suffix resolver knew four coordinates (the state, the payer, the
//! credit, the System program). The other fourteen name a live state body, a
//! record the solver or maker published, and -- for the seven escrow actions --
//! the Claims and Custody children a placement admits and a settlement moves
//! through. This module says, for every action and every runtime coordinate,
//! WHICH account belongs there, and resolves it from facts the caller states
//! or the chain holds:
//!
//! - [`general_frame_sources_v1`]: coordinate → [`GeneralFrameSourceV1`], read
//!   off the same authors the AccountProfile is emitted from
//!   (`state_artifacts_v3` for the state prefix and the evidence table,
//!   `effect_artifacts_v3` for the child routes, and the Claims/Custody frame
//!   specs for the child coordinates), so a coordinate here cannot disagree
//!   with the profile the chain holds;
//! - [`general_subject_states_v1`]: the primary/secondary/result state PDAs
//!   for an action from the subject the caller names (a batch, an order, a
//!   candidate), through the family's own seed recipes;
//! - [`GeneralEscrowChildrenV1`]: the escrow children of one order or one
//!   candidate -- the Position pair, the Custody replay, the vault -- derived
//!   from the Claims and Custody seed types, never typed;
//! - [`general_runtime_suffix_v1`]: the physical runtime suffix the route
//!   states, with the profile's own privileges per ordinal.
//!
//! The successor's session driver calls these; the program-test bundle
//! builder still carries its own request deriver (a named debt). Nothing here
//! signs, reads a chain or invents a coordinate: an input the action needs and
//! the caller did not state is refused by name.

use dclutch_claims::frame_spec_v1::{ClaimsFrameRoleV1, ClaimsFrameSpecV1};
use dclutch_claims::protocol_position_v2::{
    ProtocolPositionAdmissionSeedsV2, ProtocolPositionSeedsV2,
};
use dclutch_custody::{
    CallerRoleV1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyFrameRoleV1, CustodyFrameSpecV1,
    CustodyReplaySeedsV1, CustodyVaultSeedsV1,
};
use dclutch_market::capability_program::hot_v3::HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3;
use dclutch_trading::general::collection_v1::{
    GeneralBatchOccurrenceTermsV1, GeneralBatchOpeningV1,
};
use dclutch_trading::general::effect_artifacts_v3::{
    GeneralChildFrameV3, general_custody_callee_coordinate_v3, general_effect_route_count_v3,
    general_effect_route_frame_v3,
};
use dclutch_trading::general::state_artifacts_v3::{
    GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_TERMINAL_STATE_ACCOUNT_V3,
    GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3, GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3,
    GeneralReadonlyEvidenceKindV3, general_create_payer_account_v3,
    general_readonly_evidence_count_v3, general_readonly_evidence_v3,
    general_rent_credit_account_v3, general_system_program_account_v3,
};
use dclutch_trading::general::state_seeds_v3::{GeneralStateAddressSeedsV3, GeneralStateRecipeV3};
use dclutch_trading::general_codec::Action;
use dclutch_trading::general_config::{GeneralRootV2, v3::GeneralConfigV3};
use dclutch_vm::account_profile::v2::AliasKindV2;
use dclutch_vm::account_profile::v3::AccountProfileV3;
use solana_program::pubkey::Pubkey;
use solana_sdk_ids::{system_program, sysvar};

/// Stable refusal from General frame derivation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneralSessionErrorV1 {
    /// A family author refused a coordinate or width it owns.
    Geometry(&'static str),
    /// The action needs a subject the caller did not state.
    SubjectMissing(&'static str),
    /// The action needs an input the caller did not state.
    InputMissing(&'static str),
    /// The published AccountProfile did not decode.
    Profile,
    /// A seed tuple did not form from the stated facts.
    Seeds(&'static str),
    /// A runtime coordinate has no source this module can name.
    Unmapped(u16),
}

/// The subject one action operates on, as the caller states it.
///
/// Every field is optional because every action needs a different subset; an
/// action that needs one the caller omitted refuses `SubjectMissing` by name.
/// `batch_id` absent is NOT an error for the batch actions: it means the batch
/// the root last opened, which is the only batch `CloseBatch`, `PlaceOrder` and
/// `CancelOrder` can name while a root admits one open batch at a time.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneralSubjectV1 {
    /// The batch occurrence identity; absent means the last one the root opened.
    pub batch_id: Option<[u8; 32]>,
    /// The order identity (`GeneralSignedOrderTermsV1::order_id`).
    pub order_id: Option<[u8; 32]>,
    /// The candidate identity (the image's own digest).
    pub candidate_id: Option<[u8; 32]>,
    /// The settlement revision a `Close` consumes; its terminal record is at
    /// revision + 1, exactly as `runtime_settlement::close` names it.
    pub settlement_revision: Option<u64>,
}

/// The lifecycle states one action names, derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralSubjectStatesV1 {
    /// The primary state and its canonical bump.
    pub primary: (Pubkey, u8),
    /// The secondary state (the order for PlaceOrder/CancelOrder, the verifier
    /// cursor for VerifyCandidateRow, the terminal record for Close).
    pub secondary: Option<(Pubkey, u8)>,
    /// The conditional verified-candidate result (VerifyCandidateRow only).
    pub result: Option<(Pubkey, u8)>,
    /// The batch identity the primary or secondary state is keyed by, where
    /// the action names one; reported so a caller can print it.
    pub batch_id: Option<[u8; 32]>,
}

/// The batch occurrence the root last opened.
///
/// `OpenBatch` names sequence `next_batch_sequence`; every later batch action
/// names the one before it. The occurrence identity is slot-independent, which
/// is what makes it derivable from the root and the config alone.
pub fn general_last_opened_batch_id_v1(
    root: GeneralRootV2,
    config: GeneralConfigV3,
    config_id: [u8; 32],
    product_id: [u8; 32],
    outcome_count: u32,
) -> Result<[u8; 32], GeneralSessionErrorV1> {
    let sequence =
        root.next_batch_sequence()
            .checked_sub(1)
            .ok_or(GeneralSessionErrorV1::SubjectMissing(
                "batch: the root has opened no batch yet",
            ))?;
    general_batch_occurrence_id_v1(root, config, config_id, product_id, outcome_count, sequence)
}

/// The batch occurrence `OpenBatch` will open next.
pub fn general_next_batch_id_v1(
    root: GeneralRootV2,
    config: GeneralConfigV3,
    config_id: [u8; 32],
    product_id: [u8; 32],
    outcome_count: u32,
) -> Result<[u8; 32], GeneralSessionErrorV1> {
    general_batch_occurrence_id_v1(
        root,
        config,
        config_id,
        product_id,
        outcome_count,
        root.next_batch_sequence(),
    )
}

fn general_batch_occurrence_id_v1(
    root: GeneralRootV2,
    config: GeneralConfigV3,
    config_id: [u8; 32],
    product_id: [u8; 32],
    outcome_count: u32,
    sequence: u64,
) -> Result<[u8; 32], GeneralSessionErrorV1> {
    Ok(GeneralBatchOccurrenceTermsV1::new(GeneralBatchOpeningV1 {
        outcome_count,
        sequence,
        generation: root.generation(),
        market: root.market(),
        product_id,
        config_id,
        price_scale: config.price_scale(),
        collection_close_slot: 0,
        settlement_close_slot: 0,
        max_orders: config.max_orders_per_candidate(),
    })
    .map_err(|_| GeneralSessionErrorV1::Seeds("batch occurrence"))?
    .occurrence_id())
}

fn pda(
    seeds: GeneralStateAddressSeedsV3,
    program: Pubkey,
) -> Result<(Pubkey, u8), GeneralSessionErrorV1> {
    let slices = seeds
        .as_slices()
        .map_err(|_| GeneralSessionErrorV1::Seeds("state seed order"))?;
    Ok(Pubkey::find_program_address(slices.as_slice(), &program))
}

/// Derive the lifecycle states one action names from its stated subject.
///
/// The recipe an action's PRIMARY state uses is
/// `GeneralStateRecipeV3::primary_for_action`, the family's one author; this
/// function is only the join from the subject the caller stated to that
/// recipe's second seed. The secondary and result states are the four shapes
/// `state_artifacts_v3` declares: the order beside its batch, the verifier
/// cursor and the verified result beside a candidate, the terminal record
/// beside a settlement cursor.
#[allow(clippy::too_many_arguments)]
pub fn general_subject_states_v1(
    action: Action,
    trading: Pubkey,
    root_address: Pubkey,
    root: GeneralRootV2,
    config: GeneralConfigV3,
    config_id: [u8; 32],
    product_id: [u8; 32],
    outcome_count: u32,
    subject: GeneralSubjectV1,
) -> Result<GeneralSubjectStatesV1, GeneralSessionErrorV1> {
    let root_seed = root_address.to_bytes();
    let last_batch = || match subject.batch_id {
        Some(value) => Ok(value),
        None => general_last_opened_batch_id_v1(root, config, config_id, product_id, outcome_count),
    };
    let candidate = || {
        subject
            .candidate_id
            .ok_or(GeneralSessionErrorV1::SubjectMissing("--candidate-id"))
    };
    let order = || {
        subject
            .order_id
            .ok_or(GeneralSessionErrorV1::SubjectMissing("--order-id"))
    };
    let seeds = |recipe: GeneralStateRecipeV3, coordinate: [u8; 32]| {
        match recipe {
            GeneralStateRecipeV3::Batch => GeneralStateAddressSeedsV3::batch(root_seed, coordinate),
            GeneralStateRecipeV3::Order => GeneralStateAddressSeedsV3::order(root_seed, coordinate),
            GeneralStateRecipeV3::Candidate => {
                GeneralStateAddressSeedsV3::candidate(root_seed, coordinate)
            }
            GeneralStateRecipeV3::Selection => {
                GeneralStateAddressSeedsV3::selection(root_seed, coordinate)
            }
            GeneralStateRecipeV3::Settlement => {
                GeneralStateAddressSeedsV3::settlement(root_seed, coordinate)
            }
            GeneralStateRecipeV3::Verifier => {
                GeneralStateAddressSeedsV3::verifier(root_seed, coordinate)
            }
            GeneralStateRecipeV3::VerifiedCandidate => {
                GeneralStateAddressSeedsV3::verified_candidate(root_seed, coordinate)
            }
            GeneralStateRecipeV3::Terminal => {
                return Err(GeneralSessionErrorV1::Seeds("terminal needs a revision"));
            }
        }
        .map_err(|_| GeneralSessionErrorV1::Seeds("state recipe"))
    };
    let (primary_coordinate, secondary, result, batch_id) = match action {
        Action::OpenBatch => {
            let id = general_next_batch_id_v1(root, config, config_id, product_id, outcome_count)?;
            (id, None, None, Some(id))
        }
        Action::CloseBatch => {
            let id = last_batch()?;
            (id, None, None, Some(id))
        }
        Action::PlaceOrder | Action::CancelOrder => {
            let id = last_batch()?;
            let order_id = order()?;
            (
                id,
                Some(pda(seeds(GeneralStateRecipeV3::Order, order_id)?, trading)?),
                None,
                Some(id),
            )
        }
        Action::ReleaseOrder => (order()?, None, None, None),
        Action::SubmitCandidate | Action::CloseCandidate => (candidate()?, None, None, None),
        Action::VerifyCandidateRow => {
            let id = candidate()?;
            (
                id,
                Some(pda(seeds(GeneralStateRecipeV3::Verifier, id)?, trading)?),
                Some(pda(
                    seeds(GeneralStateRecipeV3::VerifiedCandidate, id)?,
                    trading,
                )?),
                None,
            )
        }
        // The selection is keyed by the batch the verified candidate names.
        Action::Consider | Action::Freeze => {
            let id = last_batch()?;
            (id, None, None, Some(id))
        }
        Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute => (candidate()?, None, None, None),
        Action::Close => {
            let id = candidate()?;
            let revision = subject
                .settlement_revision
                .ok_or(GeneralSessionErrorV1::SubjectMissing(
                    "--settlement-revision",
                ))?
                .checked_add(1)
                .ok_or(GeneralSessionErrorV1::Geometry("terminal revision"))?;
            let terminal = GeneralStateAddressSeedsV3::terminal(root_seed, id, revision)
                .map_err(|_| GeneralSessionErrorV1::Seeds("terminal recipe"))?;
            (id, Some(pda(terminal, trading)?), None, None)
        }
    };
    let primary = pda(
        seeds(
            GeneralStateRecipeV3::primary_for_action(action),
            primary_coordinate,
        )?,
        trading,
    )?;
    Ok(GeneralSubjectStatesV1 {
        primary,
        secondary,
        result,
        batch_id,
    })
}

/// What one runtime coordinate of a General frame IS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralFrameSourceV1 {
    /// The action's primary lifecycle state.
    PrimaryState,
    /// The secondary state (order, verifier cursor, or terminal record).
    SecondaryState,
    /// The conditional verified-candidate result.
    ResultState,
    /// The signing payer of this action (maker, solver, or cranker).
    Payer,
    /// The market's permanent RentCredit record.
    RentCredit,
    /// The solver's wallet at CloseCandidate's credit coordinate (decision
    /// 0021 `Payer`: the recorded beneficiary, paid by identity).
    SolverWallet,
    /// The System program, as an account.
    SystemProgram,
    /// One readonly evidence account of the named kind.
    Evidence(GeneralReadonlyEvidenceKindV3),
    /// One coordinate of a Claims child route.
    Claims(u16, ClaimsFrameRoleV1),
    /// One coordinate of a Custody child route.
    Custody(u16, CustodyFrameRoleV1),
    /// The release-selected Custody program the routes are invoked through.
    CustodyCallee,
}

/// Every runtime coordinate of one action, in logical order, from the same
/// authors the profile is emitted from.
pub fn general_frame_sources_v1(
    action: Action,
) -> Result<Vec<(u16, GeneralFrameSourceV1)>, GeneralSessionErrorV1> {
    let mut sources = Vec::new();
    let fixed_count =
        dclutch_trading::general::account_rules_v3::general_account_profile_fixed_count_v3(action)
            .map_err(|_| GeneralSessionErrorV1::Geometry("fixed count"))?;
    let two_state = matches!(
        action,
        Action::PlaceOrder | Action::CancelOrder | Action::Close
    );
    let payer = general_create_payer_account_v3(action)
        .unwrap_or(dclutch_trading::general::state_artifacts_v3::GENERAL_PRIMARY_PAYER_ACCOUNT_V3);
    let credit = general_rent_credit_account_v3(action);
    let system = general_system_program_account_v3(action);
    let callee = general_custody_callee_coordinate_v3(action)
        .map_err(|_| GeneralSessionErrorV1::Geometry("custody callee"))?;
    let start = u16::try_from(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3)
        .map_err(|_| GeneralSessionErrorV1::Geometry("runtime start"))?;
    let mut coordinate = start;
    while coordinate < fixed_count {
        let source = if coordinate == GENERAL_PRIMARY_STATE_ACCOUNT_V3 {
            GeneralFrameSourceV1::PrimaryState
        } else if two_state && coordinate == GENERAL_TERMINAL_STATE_ACCOUNT_V3 {
            GeneralFrameSourceV1::SecondaryState
        } else if action == Action::VerifyCandidateRow
            && coordinate == GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3
        {
            GeneralFrameSourceV1::SecondaryState
        } else if action == Action::VerifyCandidateRow
            && coordinate == GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3
        {
            GeneralFrameSourceV1::ResultState
        } else if coordinate == payer {
            GeneralFrameSourceV1::Payer
        } else if coordinate == credit {
            if action == Action::CloseCandidate {
                GeneralFrameSourceV1::SolverWallet
            } else {
                GeneralFrameSourceV1::RentCredit
            }
        } else if Some(coordinate) == system {
            GeneralFrameSourceV1::SystemProgram
        } else if Some(coordinate) == callee {
            GeneralFrameSourceV1::CustodyCallee
        } else if let Some(kind) = evidence_kind_at(action, coordinate)? {
            GeneralFrameSourceV1::Evidence(kind)
        } else if let Some(source) = child_source_at(action, coordinate)? {
            source
        } else {
            return Err(GeneralSessionErrorV1::Unmapped(coordinate));
        };
        sources.push((coordinate, source));
        coordinate = coordinate
            .checked_add(1)
            .ok_or(GeneralSessionErrorV1::Geometry("coordinate"))?;
    }
    Ok(sources)
}

fn evidence_kind_at(
    action: Action,
    coordinate: u16,
) -> Result<Option<GeneralReadonlyEvidenceKindV3>, GeneralSessionErrorV1> {
    let mut index = 0_u16;
    while index < general_readonly_evidence_count_v3(action) {
        let selected = general_readonly_evidence_v3(action, index)
            .map_err(|_| GeneralSessionErrorV1::Geometry("evidence table"))?;
        if selected.coordinate == coordinate {
            return Ok(Some(selected.kind));
        }
        index = index
            .checked_add(1)
            .ok_or(GeneralSessionErrorV1::Geometry("evidence index"))?;
    }
    Ok(None)
}

fn child_source_at(
    action: Action,
    coordinate: u16,
) -> Result<Option<GeneralFrameSourceV1>, GeneralSessionErrorV1> {
    let mut route = 0_u16;
    while route < general_effect_route_count_v3(action) {
        let frame = general_effect_route_frame_v3(action, route)
            .map_err(|_| GeneralSessionErrorV1::Geometry("route frame"))?;
        let count = frame
            .frame
            .account_count()
            .map_err(|_| GeneralSessionErrorV1::Geometry("route width"))?;
        let end = frame
            .account_start
            .checked_add(count)
            .ok_or(GeneralSessionErrorV1::Geometry("route end"))?;
        if coordinate >= frame.account_start && coordinate < end {
            let index = coordinate - frame.account_start;
            let source = match frame.frame {
                GeneralChildFrameV3::ClaimsProtocolPosition(position_action) => {
                    GeneralFrameSourceV1::Claims(
                        route,
                        ClaimsFrameSpecV1::protocol_position(position_action)
                            .account(index)
                            .map_err(|_| GeneralSessionErrorV1::Geometry("claims frame"))?
                            .role(),
                    )
                }
                GeneralChildFrameV3::ClaimsAffine { position_count } => {
                    GeneralFrameSourceV1::Claims(
                        route,
                        ClaimsFrameSpecV1::affine(position_count)
                            .and_then(|spec| spec.account(index))
                            .map_err(|_| GeneralSessionErrorV1::Geometry("affine frame"))?
                            .role(),
                    )
                }
                GeneralChildFrameV3::Custody(operation) => GeneralFrameSourceV1::Custody(
                    route,
                    CustodyFrameSpecV1::new(operation)
                        .account(index)
                        .map_err(|_| GeneralSessionErrorV1::Geometry("custody frame"))?
                        .role(),
                ),
            };
            return Ok(Some(source));
        }
        route = route
            .checked_add(1)
            .ok_or(GeneralSessionErrorV1::Geometry("route index"))?;
    }
    Ok(None)
}

/// The escrow children of one order or one candidate, derived.
///
/// Claims escrow is owned by the actual Trading Order or settlement-state
/// account. Custody replay and vault namespaces use the separate signed order
/// or candidate content identity. Native Claims/Custody seed types derive each
/// address; these two namespaces must never be substituted for one another.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEscrowChildrenV1 {
    /// Content identity keying Custody (an order or a candidate).
    pub context: [u8; 32],
    /// Actual Trading-owned Order or settlement account owning Claims escrow.
    pub position_owner: Pubkey,
    /// The Claims protocol Position owned by `position_owner`.
    pub position: Pubkey,
    /// Its admission record.
    pub admission: Pubkey,
    /// The Custody replay ledger for `context`, under the Trading caller role.
    pub replay: Pubkey,
    /// The Settlement-compartment vault for `context`.
    pub vault: Pubkey,
}

impl GeneralEscrowChildrenV1 {
    /// Derive the four children for one context under one market.
    pub fn derive(
        claims_program: Pubkey,
        custody_program: Pubkey,
        aggregate: Pubkey,
        market: [u8; 32],
        release_set: [u8; 32],
        context: [u8; 32],
        position_owner: Pubkey,
    ) -> Result<Self, GeneralSessionErrorV1> {
        let position_seeds =
            ProtocolPositionSeedsV2::new(aggregate.to_bytes(), position_owner.to_bytes())
                .map_err(|_| GeneralSessionErrorV1::Seeds("protocol position"))?;
        let admission_seeds =
            ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), position_owner.to_bytes())
                .map_err(|_| GeneralSessionErrorV1::Seeds("position admission"))?;
        let replay_seeds =
            CustodyReplaySeedsV1::new(market, release_set, CallerRoleV1::Trading, context);
        let vault_seeds =
            CustodyVaultSeedsV1::new(market, release_set, context, CompartmentV1::Settlement);
        Ok(Self {
            context,
            position_owner,
            position: Pubkey::find_program_address(&position_seeds.as_slices(), &claims_program).0,
            admission: Pubkey::find_program_address(&admission_seeds.as_slices(), &claims_program)
                .0,
            replay: Pubkey::find_program_address(&replay_seeds.as_slices(), &custody_program).0,
            vault: Pubkey::find_program_address(&vault_seeds.as_slices(), &custody_program).0,
        })
    }
}

/// The Hoard: the market's own HoardPrincipal vault, keyed by the market.
#[must_use]
pub fn general_hoard_vault_v1(
    custody_program: Pubkey,
    market: [u8; 32],
    release_set: [u8; 32],
) -> Pubkey {
    let seeds =
        CustodyVaultSeedsV1::new(market, release_set, market, CompartmentV1::HoardPrincipal);
    Pubkey::find_program_address(&seeds.as_slices(), &custody_program).0
}

/// The Custody authority PDA every vault is owned through.
#[must_use]
pub fn general_custody_authority_v1(
    custody_program: Pubkey,
    market: [u8; 32],
    release_set: [u8; 32],
) -> Pubkey {
    let seeds = CustodyAuthoritySeedsV1::new(market, release_set);
    Pubkey::find_program_address(&seeds.as_slices(), &custody_program).0
}

/// One party to an escrow action: the maker of an order, or the owner a
/// settlement row pays.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEscrowPartyV1 {
    /// The wallet that signs (maker) or is paid (distribution).
    pub owner: Pubkey,
    /// The owner's own Claims Position, admitted before any General action.
    pub position: Pubkey,
    /// The owner's own collateral token account.
    pub token_account: Pubkey,
}

/// A record the solver or maker published and the frame reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralEvidenceAddressV1 {
    /// Which evidence kind this address stands at.
    pub kind: GeneralReadonlyEvidenceKindV3,
    /// The account holding exactly the record's bytes.
    pub address: Pubkey,
}

/// The Claims and Custody facts a General child route names, and nothing else.
///
/// SEVEN OF FIFTEEN ACTIONS INVOKE A CHILD; the other eight name a state, a
/// payer, a credit and evidence, and their frames reach none of this. Holding
/// them in one optional group is what lets a caller that frames only those
/// eight -- the successor's session driver frames `OpenBatch` -- state the
/// facts it actually observed instead of inventing an aggregate, a mint and a
/// realm record to satisfy a struct. A frame that reaches one of these
/// without it refuses by name rather than reading a placeholder.
#[derive(Clone, Copy, Debug)]
pub struct GeneralChildChainV1 {
    /// Claims.
    pub claims_program: Pubkey,
    /// Claims' ProgramData.
    pub claims_programdata: Pubkey,
    /// Custody (the Custody callee).
    pub custody_program: Pubkey,
    /// The realm's token program.
    pub token_program: Pubkey,
    /// The Rent program a Claims child names beside the sysvar.
    pub rent_program: Pubkey,
    /// The Claims aggregate for the Market.
    pub aggregate: Pubkey,
    /// The collateral mint.
    pub mint: Pubkey,
    /// Realm record raw and staging.
    pub realm_record: (Pubkey, Pubkey),
}

/// Everything a General frame's runtime suffix can name, as the caller states
/// it. Chain-observed facts (programs, records, the market) are required; the
/// subject and the parties are per action and refused by name when missing.
#[derive(Clone, Debug)]
pub struct GeneralFrameInputsV1 {
    /// Programs and their ProgramData, from the plan.
    pub trading_program: Pubkey,
    /// Trading's ProgramData.
    pub trading_programdata: Pubkey,
    /// Core.
    pub core_program: Pubkey,
    /// Core's ProgramData.
    pub core_programdata: Pubkey,
    /// Registry.
    pub registry_program: Pubkey,
    /// The Registry activation cache.
    pub activation_cache: Pubkey,
    /// The Core Market.
    pub market: Pubkey,
    /// The market's permanent RentCredit record.
    pub rent_credit: Pubkey,
    /// The release set the market selected.
    pub release_set: [u8; 32],
    /// Finalized Product graph records and their staging cursors.
    pub product_record: (Pubkey, Pubkey),
    /// Result domain raw and staging.
    pub result_domain_record: (Pubkey, Pubkey),
    /// Portfolio raw and staging.
    pub portfolio_record: (Pubkey, Pubkey),
    /// Linked liability basis raw and staging.
    pub linked_basis_record: (Pubkey, Pubkey),
    /// The Claims and Custody facts only a child route reads, absent for the
    /// eight actions that invoke none.
    pub child_chain: Option<GeneralChildChainV1>,
    /// The signing payer of this action.
    pub payer: Pubkey,
    /// The derived lifecycle states.
    pub states: GeneralSubjectStatesV1,
    /// The escrow party (PlaceOrder/CancelOrder/ReleaseOrder: the maker;
    /// Collect/Distribute: the row's owner).
    pub party: Option<GeneralEscrowPartyV1>,
    /// The order's escrow children (order actions and Collect/Distribute).
    pub order_children: Option<GeneralEscrowChildrenV1>,
    /// The candidate's settlement children (the settlement actions).
    pub settlement_children: Option<GeneralEscrowChildrenV1>,
    /// The quote-surplus beneficiary a `Close` routes the remainder to.
    pub surplus_beneficiary: Option<Pubkey>,
    /// The solver's wallet (CloseCandidate's credit coordinate).
    pub solver: Option<Pubkey>,
    /// Published evidence records by kind.
    pub evidence: Vec<GeneralEvidenceAddressV1>,
    /// The child caller authorities, one per child route, in route order.
    pub child_callers: Vec<Pubkey>,
}

impl GeneralFrameInputsV1 {
    fn evidence(
        &self,
        kind: GeneralReadonlyEvidenceKindV3,
    ) -> Result<Pubkey, GeneralSessionErrorV1> {
        // Live-state evidence is derived; published records are stated.
        let states = self.states;
        let derived = match kind {
            GeneralReadonlyEvidenceKindV3::ClosedBatch => {
                // The batch a candidate names: keyed by the batch identity the
                // subject derivation reported, or the last one opened.
                return self
                    .evidence
                    .iter()
                    .find(|e| e.kind == kind)
                    .map(|e| e.address)
                    .or_else(|| self.batch_state())
                    .ok_or(GeneralSessionErrorV1::InputMissing(
                        "--evidence closed-batch=ADDRESS",
                    ));
            }
            GeneralReadonlyEvidenceKindV3::EscrowedOrder => states.secondary.map(|(k, _)| k),
            GeneralReadonlyEvidenceKindV3::RuntimeVerifier => states.secondary.map(|(k, _)| k),
            GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate
            | GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate => self
                .evidence
                .iter()
                .find(|e| e.kind == kind)
                .map(|e| e.address),
            GeneralReadonlyEvidenceKindV3::FrozenSelection => self
                .evidence
                .iter()
                .find(|e| e.kind == kind)
                .map(|e| e.address),
            _ => None,
        };
        if let Some(address) = derived {
            return Ok(address);
        }
        self.evidence
            .iter()
            .find(|e| e.kind == kind)
            .map(|e| e.address)
            .ok_or(GeneralSessionErrorV1::InputMissing(
                "--evidence KIND=ADDRESS",
            ))
    }

    fn batch_state(&self) -> Option<Pubkey> {
        self.evidence
            .iter()
            .find(|e| e.kind == GeneralReadonlyEvidenceKindV3::ClosedBatch)
            .map(|e| e.address)
    }

    fn party(&self) -> Result<GeneralEscrowPartyV1, GeneralSessionErrorV1> {
        self.party.ok_or(GeneralSessionErrorV1::InputMissing(
            "--maker / --owner party",
        ))
    }

    fn order_children(&self) -> Result<GeneralEscrowChildrenV1, GeneralSessionErrorV1> {
        self.order_children
            .ok_or(GeneralSessionErrorV1::InputMissing("order escrow children"))
    }

    fn settlement_children(&self) -> Result<GeneralEscrowChildrenV1, GeneralSessionErrorV1> {
        self.settlement_children
            .ok_or(GeneralSessionErrorV1::InputMissing("settlement children"))
    }

    fn child_chain(&self) -> Result<GeneralChildChainV1, GeneralSessionErrorV1> {
        self.child_chain.ok_or(GeneralSessionErrorV1::InputMissing(
            "child-route chain frame",
        ))
    }

    fn child_caller(&self, route: u16) -> Result<Pubkey, GeneralSessionErrorV1> {
        self.child_callers.get(usize::from(route)).copied().ok_or(
            GeneralSessionErrorV1::InputMissing("--child-caller ROUTE=ADDRESS"),
        )
    }

    /// Resolve one frame source to its address for `action`.
    #[allow(clippy::too_many_lines)]
    pub fn resolve(
        &self,
        action: Action,
        source: GeneralFrameSourceV1,
    ) -> Result<Pubkey, GeneralSessionErrorV1> {
        let market = self.market.to_bytes();
        Ok(match source {
            GeneralFrameSourceV1::PrimaryState => self.states.primary.0,
            GeneralFrameSourceV1::SecondaryState => self
                .states
                .secondary
                .map(|(k, _)| k)
                .ok_or(GeneralSessionErrorV1::SubjectMissing("secondary state"))?,
            GeneralFrameSourceV1::ResultState => self
                .states
                .result
                .map(|(k, _)| k)
                .ok_or(GeneralSessionErrorV1::SubjectMissing("result state"))?,
            GeneralFrameSourceV1::Payer => self.payer,
            GeneralFrameSourceV1::RentCredit => self.rent_credit,
            GeneralFrameSourceV1::SolverWallet => self
                .solver
                .ok_or(GeneralSessionErrorV1::InputMissing("--solver"))?,
            GeneralFrameSourceV1::SystemProgram => system_program::ID,
            GeneralFrameSourceV1::CustodyCallee => self.child_chain()?.custody_program,
            GeneralFrameSourceV1::Evidence(kind) => self.evidence(kind)?,
            GeneralFrameSourceV1::Claims(route, role) => match role {
                ClaimsFrameRoleV1::CallerAuthority => self.child_caller(route)?,
                ClaimsFrameRoleV1::ClaimsMarket => self.child_chain()?.aggregate,
                ClaimsFrameRoleV1::ProtocolPosition => self.escrow_children(action)?.position,
                ClaimsFrameRoleV1::ProtocolPositionAdmission => {
                    self.escrow_children(action)?.admission
                }
                ClaimsFrameRoleV1::BasisRecord => self.linked_basis_record.0,
                ClaimsFrameRoleV1::BasisStaging => self.linked_basis_record.1,
                ClaimsFrameRoleV1::ProductRecord => self.product_record.0,
                ClaimsFrameRoleV1::ProductStaging => self.product_record.1,
                ClaimsFrameRoleV1::ResultDomainRecord => self.result_domain_record.0,
                ClaimsFrameRoleV1::ResultDomainStaging => self.result_domain_record.1,
                ClaimsFrameRoleV1::PortfolioRecord => self.portfolio_record.0,
                ClaimsFrameRoleV1::PortfolioStaging => self.portfolio_record.1,
                ClaimsFrameRoleV1::RentSysvar => sysvar::rent::ID,
                ClaimsFrameRoleV1::SystemProgram => system_program::ID,
                ClaimsFrameRoleV1::CoreMarket => self.market,
                ClaimsFrameRoleV1::ActivationCache => self.activation_cache,
                ClaimsFrameRoleV1::RegistryProgram => self.registry_program,
                ClaimsFrameRoleV1::TradingProgram | ClaimsFrameRoleV1::CallerProgram => {
                    self.trading_program
                }
                ClaimsFrameRoleV1::TradingProgramData | ClaimsFrameRoleV1::CallerProgramData => {
                    self.trading_programdata
                }
                ClaimsFrameRoleV1::ClaimsProgram => self.child_chain()?.claims_program,
                ClaimsFrameRoleV1::ClaimsProgramData => self.child_chain()?.claims_programdata,
                ClaimsFrameRoleV1::CoreProgram => self.core_program,
                ClaimsFrameRoleV1::CoreProgramData => self.core_programdata,
                ClaimsFrameRoleV1::PositionOwnerIdentity => match action {
                    Action::PlaceOrder | Action::CancelOrder | Action::ReleaseOrder => {
                        self.order_children()?.position_owner
                    }
                    _ => self.settlement_children()?.position_owner,
                },
                ClaimsFrameRoleV1::RentCredit => self.rent_credit,
                ClaimsFrameRoleV1::RentProgram => self.child_chain()?.rent_program,
                ClaimsFrameRoleV1::AffinePosition(index) => self.affine_position(action, index)?,
                ClaimsFrameRoleV1::SignedDeltaPosition(_)
                | ClaimsFrameRoleV1::SparseSourcePosition
                | ClaimsFrameRoleV1::SparseDestinationPosition => {
                    return Err(GeneralSessionErrorV1::Geometry(
                        "a Claims frame General never invokes",
                    ));
                }
            },
            GeneralFrameSourceV1::Custody(route, role) => match role {
                CustodyFrameRoleV1::CallerAuthority => self.child_caller(route)?,
                CustodyFrameRoleV1::CoreMarket => self.market,
                CustodyFrameRoleV1::ActivationCache => self.activation_cache,
                CustodyFrameRoleV1::RegistryProgram => self.registry_program,
                CustodyFrameRoleV1::CallerProgram => self.trading_program,
                CustodyFrameRoleV1::CallerProgramData => self.trading_programdata,
                CustodyFrameRoleV1::RealmRecord => self.child_chain()?.realm_record.0,
                CustodyFrameRoleV1::RealmStaging => self.child_chain()?.realm_record.1,
                CustodyFrameRoleV1::Replay => self.escrow_children(action)?.replay,
                CustodyFrameRoleV1::Payer => self.payer,
                CustodyFrameRoleV1::SystemProgram => system_program::ID,
                CustodyFrameRoleV1::RentSysvar => sysvar::rent::ID,
                // An order's replay and vault refund their rent to the maker
                // (`environment.rent_refund == owner` in the cancel and
                // release projectors); a settlement's to the market's credit.
                CustodyFrameRoleV1::RentRefund => match action {
                    Action::PlaceOrder | Action::CancelOrder | Action::ReleaseOrder => {
                        self.party()?.owner
                    }
                    _ => self.rent_credit,
                },
                CustodyFrameRoleV1::Mint => self.child_chain()?.mint,
                CustodyFrameRoleV1::Vault => self.escrow_children(action)?.vault,
                CustodyFrameRoleV1::CustodyAuthority => general_custody_authority_v1(
                    self.child_chain()?.custody_program,
                    market,
                    self.release_set,
                ),
                CustodyFrameRoleV1::TokenProgram => self.child_chain()?.token_program,
                CustodyFrameRoleV1::TransferSource => self.transfer_side(action, true)?,
                CustodyFrameRoleV1::TransferDestination => self.transfer_side(action, false)?,
            },
        })
    }

    /// Which escrow children a Claims/Custody child of `action` names.
    fn escrow_children(
        &self,
        action: Action,
    ) -> Result<GeneralEscrowChildrenV1, GeneralSessionErrorV1> {
        match action {
            Action::PlaceOrder | Action::CancelOrder | Action::ReleaseOrder => {
                self.order_children()
            }
            _ => self.settlement_children(),
        }
    }

    /// The two Positions of an affine child, in key order (the runtime sorts
    /// the table by address, `le_numeric_id`).
    fn affine_position(&self, action: Action, index: u16) -> Result<Pubkey, GeneralSessionErrorV1> {
        let pair: Vec<Pubkey> = match action {
            Action::PlaceOrder | Action::CancelOrder | Action::ReleaseOrder => {
                vec![self.party()?.position, self.order_children()?.position]
            }
            Action::Collect | Action::Distribute => {
                vec![self.party()?.position, self.settlement_children()?.position]
            }
            Action::Materialize => vec![self.settlement_children()?.position],
            _ => {
                return Err(GeneralSessionErrorV1::Geometry(
                    "affine on a non-affine action",
                ));
            }
        };
        let mut sorted = pair;
        sorted.sort_unstable_by_key(Pubkey::to_bytes);
        sorted
            .get(usize::from(index))
            .copied()
            .ok_or(GeneralSessionErrorV1::Geometry("affine position index"))
    }

    /// The Custody transfer's source or destination for `action`, from
    /// `general_child_custody_movement_v1`'s compartment table restated as
    /// addresses: order escrow ⇄ maker at placement/refund, order escrow →
    /// settlement at Collect, settlement ⇄ Hoard at Materialize, settlement →
    /// owner at Distribute, settlement → surplus beneficiary at Close.
    fn transfer_side(&self, action: Action, source: bool) -> Result<Pubkey, GeneralSessionErrorV1> {
        let hoard = general_hoard_vault_v1(
            self.child_chain()?.custody_program,
            self.market.to_bytes(),
            self.release_set,
        );
        Ok(match (action, source) {
            (Action::PlaceOrder, true) => self.party()?.token_account,
            (Action::PlaceOrder, false) => self.order_children()?.vault,
            (Action::CancelOrder | Action::ReleaseOrder, true) => self.order_children()?.vault,
            (Action::CancelOrder | Action::ReleaseOrder, false) => self.party()?.token_account,
            (Action::Collect, true) => self.order_children()?.vault,
            (Action::Collect, false) => self.settlement_children()?.vault,
            // The complete-set move's direction is the candidate's; the
            // frame carries both sides and the Effect selects which debits.
            (Action::Materialize, true) => self.settlement_children()?.vault,
            (Action::Materialize, false) => hoard,
            (Action::Distribute, true) => self.settlement_children()?.vault,
            (Action::Distribute, false) => self.party()?.token_account,
            (Action::Close, true) => self.settlement_children()?.vault,
            (Action::Close, false) => self
                .surplus_beneficiary
                .ok_or(GeneralSessionErrorV1::InputMissing("--surplus-beneficiary"))?,
            _ => {
                return Err(GeneralSessionErrorV1::Geometry(
                    "transfer on a non-transfer action",
                ));
            }
        })
    }
}

/// One physical runtime-suffix account the route states.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralRuntimeAccountV1 {
    /// The logical coordinate this physical ordinal represents.
    pub coordinate: u16,
    /// Its address.
    pub address: Pubkey,
    /// Signer bit, from the profile's own rule.
    pub is_signer: bool,
    /// Writable bit, from the profile's own rule.
    pub is_writable: bool,
    /// What it is, for the frame report.
    pub source: GeneralFrameSourceV1,
}

/// The physical runtime suffix of one action: every logical coordinate past
/// the five fixed runtime accounts whose rule is not a route alias, resolved,
/// with the privileges the published profile declares.
pub fn general_runtime_suffix_v1(
    published_profile: &[u8],
    action: Action,
    inputs: &GeneralFrameInputsV1,
) -> Result<Vec<GeneralRuntimeAccountV1>, GeneralSessionErrorV1> {
    let profile =
        AccountProfileV3::decode(published_profile).map_err(|_| GeneralSessionErrorV1::Profile)?;
    let base = profile.base();
    let mut accounts = Vec::new();
    for (coordinate, source) in general_frame_sources_v1(action)? {
        let rule = base
            .rule(false, coordinate)
            .map_err(|_| GeneralSessionErrorV1::Profile)?;
        if rule.alias_kind() == AliasKindV2::Fixed {
            // An alias is supplied by its representative; the physical list
            // does not carry it.
            continue;
        }
        let privileges = rule.privileges();
        accounts.push(GeneralRuntimeAccountV1 {
            coordinate,
            address: inputs.resolve(action, source)?,
            is_signer: privileges & 1 != 0,
            is_writable: privileges & 2 != 0,
            source,
        });
    }
    Ok(accounts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every action's runtime frame names every coordinate: no `Unmapped`.
    #[test]
    fn every_action_names_every_runtime_coordinate() {
        for action in dclutch_trading::general::release_v3::GENERAL_ACTIONS_V5 {
            let sources = general_frame_sources_v1(action)
                .unwrap_or_else(|error| panic!("{action:?}: {error:?}"));
            let fixed =
                dclutch_trading::general::account_rules_v3::general_account_profile_fixed_count_v3(
                    action,
                )
                .expect("fixed count");
            assert_eq!(
                sources.len(),
                usize::from(fixed) - HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3,
                "{action:?}"
            );
            assert_eq!(
                sources.first().map(|(_, s)| *s),
                Some(GeneralFrameSourceV1::PrimaryState),
                "{action:?}"
            );
        }
    }

    /// The batch actions share one subject derivation and OpenBatch names the
    /// batch AFTER the one CloseBatch names.
    #[test]
    fn open_batch_names_the_next_occurrence_and_close_batch_the_last() {
        let root = GeneralRootV2::active([1; 32], [2; 32], 3).expect("root");
        let mut opened = root;
        opened
            .open_batch(root.revision(), root.next_batch_sequence())
            .expect("open one batch");
        let config = dclutch_trading::general_config::v3::GeneralConfigV3::new(
            dclutch_trading::general_config::v3::GeneralConfigV3Input {
                capacity_profile_id: [4; 32],
                claim_basis_id: [5; 32],
                program_set_id: [6; 32],
                generation: 3,
                price_scale: 1_000_000,
                collection_slots: 10,
                selection_slots: 10,
                settlement_slots: 10,
                max_orders_per_candidate: 4,
                max_pages_per_candidate: 4,
                continuation_reward_lamports: 1,
                selection_policy_id: [7; 32],
                quote_surplus_beneficiary: [8; 32],
            },
        )
        .expect("config");
        let next = general_next_batch_id_v1(root, config, [2; 32], [9; 32], 2).expect("next");
        let last = general_last_opened_batch_id_v1(opened, config, [2; 32], [9; 32], 2)
            .expect("last opened");
        assert_eq!(next, last);
        assert_eq!(
            general_last_opened_batch_id_v1(root, config, [2; 32], [9; 32], 2),
            Err(GeneralSessionErrorV1::SubjectMissing(
                "batch: the root has opened no batch yet"
            ))
        );
    }

    /// An escrow's four children are four distinct PDAs under two programs.
    #[test]
    fn escrow_children_are_distinct_and_keyed_by_context() {
        let claims = Pubkey::new_from_array([0xc1; 32]);
        let custody = Pubkey::new_from_array([0xc2; 32]);
        let a = GeneralEscrowChildrenV1::derive(
            claims,
            custody,
            Pubkey::new_from_array([3; 32]),
            [1; 32],
            [2; 32],
            [0xaa; 32],
            Pubkey::new_from_array([0xca; 32]),
        )
        .expect("children");
        let b = GeneralEscrowChildrenV1::derive(
            claims,
            custody,
            Pubkey::new_from_array([3; 32]),
            [1; 32],
            [2; 32],
            [0xbb; 32],
            Pubkey::new_from_array([0xcb; 32]),
        )
        .expect("children");
        let set = std::collections::BTreeSet::from([a.position, a.admission, a.replay, a.vault]);
        assert_eq!(set.len(), 4);
        assert_ne!(a.vault, b.vault);
        assert_ne!(a.position, b.position);
        let different_owner = GeneralEscrowChildrenV1::derive(
            claims,
            custody,
            Pubkey::new_from_array([3; 32]),
            [1; 32],
            [2; 32],
            a.context,
            b.position_owner,
        )
        .expect("same Custody context with another actual Claims owner");
        assert_eq!(a.replay, different_owner.replay);
        assert_eq!(a.vault, different_owner.vault);
        assert_ne!(a.position, different_owner.position);
        assert_ne!(a.admission, different_owner.admission);
    }
}
