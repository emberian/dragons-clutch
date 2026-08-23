// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-bound authentication for general-Epoch retirement families.
//!
//! This module deliberately has no generic public "attestation" constructor.
//! A class witness can be minted only inside this adapter after both an exact
//! runtime account/absence check and the owning family's semantic terminality
//! check. Today the General V2 Window and immutable Market binding have both
//! halves, while ClearWork has exact runtime authentication but no terminal
//! disposition. The fresh counted Epoch and explicit-liability FinalPot have
//! both halves. The remaining child capabilities stay unmintable until their
//! fresh codecs and semantic owners expose the required proofs.

use clutch_general_v2_contract::{
    CandidateWindowV4AccountV1, ClearWorkHeaderV2, GeneralEpochV6AccountV1,
    GeneralV2EpochChildCountsV1, GeneralV2EpochChildKindV1, GeneralV2FinalPotV1AccountV1,
    MarketBindingV1, FINAL_POT_ACCOUNT_BYTES, FINAL_POT_ACCOUNT_TAG, FINAL_POT_ACCOUNT_VERSION,
    GENERAL_EPOCH_ACCOUNT_BYTES, GENERAL_EPOCH_ACCOUNT_TAG, GENERAL_EPOCH_ACCOUNT_VERSION,
    MARKET_BINDING_ACCOUNT_BYTES, MARKET_BINDING_ACCOUNT_TAG, MARKET_BINDING_ACCOUNT_VERSION,
    MAX_OUTCOMES, WINDOW_ACCOUNT_BYTES, WINDOW_ACCOUNT_TAG, WINDOW_ACCOUNT_VERSION,
};
use clutch_retirement::{DeletableRentOwnerV1, Identity32V1, RetirementErrorV2};

use crate::account_auth::{
    authenticate_epoch_budget_v1_exact, AbsentAccountViewV1, AccountAccessV2, AccountViewV2,
    CanonicalPdaV1,
};
use crate::RetirementAdapterErrorV2;

const WINDOW_STORED_BUMP_OFFSET: usize = 563;
const MARKET_BINDING_STORED_BUMP_OFFSET: usize = 538;
const EPOCH_CHILD_CLASS_CAPACITY_V2: usize = 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactFamilyAccountV1<'a> {
    address: Identity32V1,
    program_id: Identity32V1,
    data: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExactFamilySchemaV1 {
    tag: u8,
    version: u8,
    len: usize,
    stored_bump_offset: usize,
}

fn authenticate_exact_family_account_v1<'a>(
    view: AccountViewV2<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    access: AccountAccessV2,
    schema: ExactFamilySchemaV1,
) -> Result<ExactFamilyAccountV1<'a>, RetirementAdapterErrorV2> {
    if schema.len < 2 || schema.stored_bump_offset >= schema.len {
        return Err(RetirementAdapterErrorV2::InvalidSchema);
    }
    if view.address != canonical_pda.address() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if view.owner != program_id {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if view.is_executable {
        return Err(RetirementAdapterErrorV2::ExecutableAccount);
    }
    match access {
        AccountAccessV2::ReadOnly if view.is_writable => {
            return Err(RetirementAdapterErrorV2::UnexpectedWritable)
        }
        AccountAccessV2::Writable if !view.is_writable => {
            return Err(RetirementAdapterErrorV2::NotWritable)
        }
        AccountAccessV2::ReadOnly | AccountAccessV2::Writable => {}
    }
    if view.data.len() < schema.len {
        return Err(RetirementErrorV2::Truncated.into());
    }
    if view.data.len() > schema.len {
        return Err(RetirementErrorV2::TrailingBytes.into());
    }
    if view.data[0] != schema.tag {
        return Err(RetirementErrorV2::WrongTag.into());
    }
    if view.data[1] != schema.version {
        return Err(RetirementErrorV2::WrongVersion.into());
    }
    if view.data[schema.stored_bump_offset] != canonical_pda.bump() {
        return Err(RetirementAdapterErrorV2::WrongBump);
    }
    Ok(ExactFamilyAccountV1 {
        address: view.address,
        program_id,
        data: view.data,
    })
}

fn identity(
    value: clutch_general_v2_contract::Id32,
) -> Result<Identity32V1, RetirementAdapterErrorV2> {
    Identity32V1::new(value.bytes()).map_err(Into::into)
}

fn retirement_rent(
    value: clutch_general_v2_contract::DeletableRentOwnerV1,
) -> Result<DeletableRentOwnerV1, RetirementAdapterErrorV2> {
    Ok(DeletableRentOwnerV1::from_persisted(
        identity(value.payer)?,
        value.refundable_principal,
        value.donation_floor,
    )?)
}

/// Exact authenticated General V2 Window deletion and terminal-ledger facts.
///
/// The private fields are derived from the authoritative 565-byte Window V4
/// codec and its semantic-owner `retirement_disposition`.  This capability is
/// still not root-close authority: Budget disposition, all child-family
/// proofs, neutral-sink provenance, recipient balances, and runtime mutation
/// ordering remain separate requirements.  In particular, its parent is the
/// General V2 Epoch PDA; it is never lowered into the legacy
/// `EpochWindowRootSiblingV1` semantic-Epoch shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralV2WindowRetirementV1 {
    account: Identity32V1,
    program_id: Identity32V1,
    parent_epoch_account: Identity32V1,
    market: Identity32V1,
    epoch_generation: u64,
    rent: DeletableRentOwnerV1,
    admitted_count: u64,
    closed_node_count: u64,
    selected_candidate_artifact: [u8; 32],
}

impl AuthenticatedGeneralV2WindowRetirementV1 {
    /// Canonical Window PDA whose exact bytes were authenticated.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Exact program owner authenticated for Window.
    pub const fn program_id(self) -> Identity32V1 {
        self.program_id
    }

    /// Canonical parent Epoch PDA stored by Window V4.
    pub const fn parent_epoch_account(self) -> Identity32V1 {
        self.parent_epoch_account
    }

    /// Parent Market stored by the exact Window body.
    pub const fn market(self) -> Identity32V1 {
        self.market
    }

    /// Parent generation stored by the exact Window body.
    pub const fn epoch_generation(self) -> u64 {
        self.epoch_generation
    }

    /// Independently funded Window deletion owner.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Total Window admissions proven by the semantic owner.
    pub const fn admitted_count(self) -> u64 {
        self.admitted_count
    }

    /// Total reverse-linked closes proven by the semantic owner.
    pub const fn closed_node_count(self) -> u64 {
        self.closed_node_count
    }

    /// Historical selected-artifact bytes; zero means none was materialized.
    pub const fn selected_candidate_artifact(self) -> [u8; 32] {
        self.selected_candidate_artifact
    }
}

/// Authenticate the mandatory writable Window V4 root sibling.
///
/// This authenticates only the Window-owned facts. The future General V2
/// Epoch-family adapter must independently authenticate its fresh root codec
/// and cross-bind its account address, Market, and generation to this private
/// capability. It must not substitute the legacy Epoch V5 semantic projection.
pub fn authenticate_general_v2_window_retirement_v1(
    view: AccountViewV2<'_>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedGeneralV2WindowRetirementV1, RetirementAdapterErrorV2> {
    let account = authenticate_exact_family_account_v1(
        view,
        program_id,
        canonical_pda,
        AccountAccessV2::Writable,
        ExactFamilySchemaV1 {
            tag: WINDOW_ACCOUNT_TAG,
            version: WINDOW_ACCOUNT_VERSION,
            len: WINDOW_ACCOUNT_BYTES,
            stored_bump_offset: WINDOW_STORED_BUMP_OFFSET,
        },
    )?;
    let disposition = CandidateWindowV4AccountV1::decode(account.data)?.retirement_disposition()?;
    let parent_epoch_account = identity(disposition.epoch_account())?;

    Ok(AuthenticatedGeneralV2WindowRetirementV1 {
        account: account.address,
        program_id: account.program_id,
        parent_epoch_account,
        market: identity(disposition.market())?,
        epoch_generation: disposition.epoch_generation(),
        rent: retirement_rent(disposition.rent())?,
        admitted_count: disposition.admitted_count(),
        closed_node_count: disposition.closed_node_count(),
        selected_candidate_artifact: disposition.selected_candidate_artifact().bytes(),
    })
}

/// Exact authenticated General V2 Budget terminal disposition.
///
/// Parent `epoch_account` is the fresh General V2 Epoch PDA stored by Budget,
/// not the legacy semantic Epoch id used by the disabled Epoch V5 planner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralV2BudgetRetirementV2 {
    account: Identity32V1,
    program_id: Identity32V1,
    market: Identity32V1,
    epoch_account: Identity32V1,
    epoch_generation: u64,
    funding_payer: Identity32V1,
    root_close_reward: u64,
    rent: DeletableRentOwnerV1,
}

impl AuthenticatedGeneralV2BudgetRetirementV2 {
    /// Canonical writable Budget PDA.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Exact program owner authenticated for Budget.
    pub const fn program_id(self) -> Identity32V1 {
        self.program_id
    }

    /// Parent Market stored by Budget.
    pub const fn market(self) -> Identity32V1 {
        self.market
    }

    /// Parent General V2 Epoch account stored by Budget.
    pub const fn epoch_account(self) -> Identity32V1 {
        self.epoch_account
    }

    /// Parent generation stored by Budget.
    pub const fn epoch_generation(self) -> u64 {
        self.epoch_generation
    }

    /// Payer that capitalized root rewards and selected-artifact rent.
    pub const fn funding_payer(self) -> Identity32V1 {
        self.funding_payer
    }

    /// Still-present permissionless root-close reward.
    pub const fn root_close_reward(self) -> u64 {
        self.root_close_reward
    }

    /// Independently funded Budget deletion owner.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

/// Authenticate exact Budget bytes and consume its semantic owner's terminal
/// disposition without lowering the General V2 parent identity into a legacy
/// Epoch-root DTO.
pub fn authenticate_general_v2_budget_retirement_v2(
    view: AccountViewV2<'_>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedGeneralV2BudgetRetirementV2, RetirementAdapterErrorV2> {
    let account = authenticate_epoch_budget_v1_exact(
        view,
        program_id,
        canonical_pda,
        AccountAccessV2::Writable,
    )?;
    let disposition = clutch_general_v2_contract::EpochBudgetV2AccountV1::decode(account.data())?
        .retirement_disposition()?;
    Ok(AuthenticatedGeneralV2BudgetRetirementV2 {
        account: account.address(),
        program_id: account.program_id(),
        market: identity(disposition.market())?,
        epoch_account: identity(disposition.epoch_account())?,
        epoch_generation: disposition.epoch_generation(),
        funding_payer: identity(disposition.funding_payer())?,
        root_close_reward: disposition.root_close_reward(),
        rent: retirement_rent(disposition.rent())?,
    })
}

/// Exact authenticated immutable Market-binding neutral-sink provenance.
///
/// This capability proves the binding account's runtime owner/PDA/header/
/// length/bump/read-only/non-executable facts and delegates all body semantics
/// to `MarketBindingV1`.  It does not prove that an arbitrary Epoch belongs to
/// this Market; the root join must compare its private Market identity.  It is
/// intentionally not lowered into the legacy forgeable neutral-sink DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralV2NeutralSinkBindingV1 {
    account: Identity32V1,
    program_id: Identity32V1,
    market: Identity32V1,
    neutral_sink: Identity32V1,
}

impl AuthenticatedGeneralV2NeutralSinkBindingV1 {
    /// Canonical immutable Market-binding PDA.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Exact program owner authenticated for MarketBinding.
    pub const fn program_id(self) -> Identity32V1 {
        self.program_id
    }

    /// Market identity stored by the authoritative binding codec.
    pub const fn market(self) -> Identity32V1 {
        self.market
    }

    /// Immutable neutral sink stored by the authoritative binding codec.
    pub const fn neutral_sink(self) -> Identity32V1 {
        self.neutral_sink
    }
}

/// Authenticate one immutable read-only General V2 Market binding.
pub fn authenticate_general_v2_neutral_sink_binding_v1(
    view: AccountViewV2<'_>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedGeneralV2NeutralSinkBindingV1, RetirementAdapterErrorV2> {
    let account = authenticate_exact_family_account_v1(
        view,
        program_id,
        canonical_pda,
        AccountAccessV2::ReadOnly,
        ExactFamilySchemaV1 {
            tag: MARKET_BINDING_ACCOUNT_TAG,
            version: MARKET_BINDING_ACCOUNT_VERSION,
            len: MARKET_BINDING_ACCOUNT_BYTES,
            stored_bump_offset: MARKET_BINDING_STORED_BUMP_OFFSET,
        },
    )?;
    let binding = MarketBindingV1::decode(account.data)?;
    Ok(AuthenticatedGeneralV2NeutralSinkBindingV1 {
        account: account.address,
        program_id: account.program_id,
        market: identity(binding.market)?,
        neutral_sink: identity(binding.neutral_sink)?,
    })
}

/// Authenticated mandatory sibling join for the fresh General V2 root family.
///
/// This is not a root-close capability. It proves only Window, Budget, and
/// immutable neutral-sink provenance. The fresh Epoch root codec and every
/// counted child-family disposition remain mandatory separate inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralV2RootSiblingsV1 {
    window: AuthenticatedGeneralV2WindowRetirementV1,
    budget: AuthenticatedGeneralV2BudgetRetirementV2,
    neutral_sink: AuthenticatedGeneralV2NeutralSinkBindingV1,
}

impl AuthenticatedGeneralV2RootSiblingsV1 {
    /// Exact Window retirement capability.
    pub const fn window(self) -> AuthenticatedGeneralV2WindowRetirementV1 {
        self.window
    }

    /// Exact Budget retirement capability.
    pub const fn budget(self) -> AuthenticatedGeneralV2BudgetRetirementV2 {
        self.budget
    }

    /// Exact immutable neutral-sink capability.
    pub const fn neutral_sink(self) -> AuthenticatedGeneralV2NeutralSinkBindingV1 {
        self.neutral_sink
    }
}

/// Cross-bind the three currently implemented General V2 root prerequisites.
pub fn authenticate_general_v2_root_siblings_v1(
    window: AuthenticatedGeneralV2WindowRetirementV1,
    budget: AuthenticatedGeneralV2BudgetRetirementV2,
    neutral_sink: AuthenticatedGeneralV2NeutralSinkBindingV1,
) -> Result<AuthenticatedGeneralV2RootSiblingsV1, RetirementAdapterErrorV2> {
    if window.program_id != budget.program_id
        || window.program_id != neutral_sink.program_id
        || window.market != budget.market
        || window.market != neutral_sink.market
        || window.parent_epoch_account != budget.epoch_account
    {
        return Err(RetirementErrorV2::WrongParent.into());
    }
    if window.epoch_generation != budget.epoch_generation {
        return Err(RetirementErrorV2::WrongGeneration.into());
    }
    if window.account == budget.account
        || neutral_sink.account == neutral_sink.neutral_sink
        || window.account == neutral_sink.neutral_sink
        || budget.account == neutral_sink.neutral_sink
        || window.rent.payer() == window.account
        || window.rent.payer() == budget.account
        || budget.rent.payer() == window.account
        || budget.rent.payer() == budget.account
        || window.rent.payer() == neutral_sink.neutral_sink
        || budget.rent.payer() == neutral_sink.neutral_sink
        || budget.funding_payer == window.account
        || budget.funding_payer == budget.account
        || budget.funding_payer == neutral_sink.neutral_sink
    {
        return Err(RetirementErrorV2::AccountAlias.into());
    }
    Ok(AuthenticatedGeneralV2RootSiblingsV1 {
        window,
        budget,
        neutral_sink,
    })
}

/// Fresh General V2 parent identity shared by every counted child capability.
///
/// `epoch_account` is always the full General V2 Epoch PDA. It is never the
/// legacy Epoch V5 semantic identifier. Private fields prevent a caller from
/// rebinding an owner-issued child capability to another generation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralV2EpochChildParentV1 {
    program_id: Identity32V1,
    market: Identity32V1,
    epoch_account: Identity32V1,
    epoch_generation: u64,
}

impl GeneralV2EpochChildParentV1 {
    /// Exact program owner authenticated for the family account.
    pub const fn program_id(self) -> Identity32V1 {
        self.program_id
    }

    /// General V2 Market identity stored by the family owner.
    pub const fn market(self) -> Identity32V1 {
        self.market
    }

    /// Fresh General V2 Epoch PDA stored by the family owner.
    pub const fn epoch_account(self) -> Identity32V1 {
        self.epoch_account
    }

    /// Nonzero parent generation stored by the family owner.
    pub const fn epoch_generation(self) -> u64 {
        self.epoch_generation
    }
}

/// Exact terminal fresh General V2 Epoch root required by every family join.
///
/// A legacy Epoch V5 account cannot mint this capability. Only the exact fresh
/// General V2 Epoch codec, canonical PDA, owner, writable role, stored bump,
/// terminal phase, generation, and nine zero authoritative counters do so.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralV2TerminalEpochV1 {
    parent: GeneralV2EpochChildParentV1,
    children: GeneralV2EpochChildCountsV1,
}

impl AuthenticatedGeneralV2TerminalEpochV1 {
    /// Exact program/Market/fresh-Epoch-PDA/generation binding.
    pub const fn parent(self) -> GeneralV2EpochChildParentV1 {
        self.parent
    }

    /// All nine authoritative child counters from the fresh Epoch owner.
    pub const fn children(self) -> GeneralV2EpochChildCountsV1 {
        self.children
    }
}

/// Authenticate the exact settlement-complete General V2 Epoch root and mint
/// its terminal capability only after all nine authoritative counts are zero.
pub fn authenticate_general_v2_terminal_epoch_v1(
    view: AccountViewV2<'_>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedGeneralV2TerminalEpochV1, RetirementAdapterErrorV2> {
    let authenticated = authenticate_exact_family_account_v1(
        view,
        program_id,
        canonical_pda,
        AccountAccessV2::Writable,
        ExactFamilySchemaV1 {
            tag: GENERAL_EPOCH_ACCOUNT_TAG,
            version: GENERAL_EPOCH_ACCOUNT_VERSION,
            len: GENERAL_EPOCH_ACCOUNT_BYTES,
            stored_bump_offset: GENERAL_EPOCH_ACCOUNT_BYTES - 2,
        },
    )?;
    let epoch = GeneralEpochV6AccountV1::decode(authenticated.data)?;
    let disposition = epoch.retirement_disposition()?;
    if disposition.stored_bump() != canonical_pda.bump() {
        return Err(RetirementAdapterErrorV2::WrongBump);
    }
    Ok(AuthenticatedGeneralV2TerminalEpochV1 {
        parent: GeneralV2EpochChildParentV1 {
            program_id,
            market: identity(epoch.market_runtime)?,
            epoch_account: authenticated.address,
            epoch_generation: epoch.generation,
        },
        children: epoch.children,
    })
}

/// Exact authenticated General V2 ClearWork V2 runtime state.
///
/// This is deliberately not a terminal or close capability. The authoritative
/// codec proves dynamic length, canonical tail padding, phase cursors, reward
/// conservation, and the fresh Epoch-PDA/generation binding. General V2 still
/// lacks a semantic-owner retirement disposition proving how the remaining
/// reward and rent compartments close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralV2ClearWorkAccountV1 {
    account: Identity32V1,
    parent: GeneralV2EpochChildParentV1,
    node: Identity32V1,
    order_set: Identity32V1,
    feed: Identity32V1,
    settlement_candidate: Identity32V1,
    rent: DeletableRentOwnerV1,
    reward_remaining: u64,
    reward_earned: u64,
    phase: u8,
}

impl AuthenticatedGeneralV2ClearWorkAccountV1 {
    /// Canonical writable ClearWork PDA.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Exact fresh parent identity decoded from ClearWork.
    pub const fn parent(self) -> GeneralV2EpochChildParentV1 {
        self.parent
    }

    /// AdmissionNode owning this work account.
    pub const fn node(self) -> Identity32V1 {
        self.node
    }

    /// Frozen order-set identity checked by the semantic owner.
    pub const fn order_set(self) -> Identity32V1 {
        self.order_set
    }

    /// Canonical candidate Feed identity checked by the semantic owner.
    pub const fn feed(self) -> Identity32V1 {
        self.feed
    }

    /// Typed selected-candidate identity checked by the semantic owner.
    pub const fn settlement_candidate(self) -> Identity32V1 {
        self.settlement_candidate
    }

    /// Independently funded work-account rent compartment.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Exact unpaid reward reserve; no close disposition is inferred from it.
    pub const fn reward_remaining(self) -> u64 {
        self.reward_remaining
    }

    /// Exact monotonically earned rewards.
    pub const fn reward_earned(self) -> u64 {
        self.reward_earned
    }

    /// Authoritative work phase, retained without interpreting it as closable.
    pub const fn phase(self) -> u8 {
        self.phase
    }
}

/// Authenticate the dynamic-width General V2 ClearWork V2 account exactly.
///
/// The authoritative decoder owns header/version, active-width calculation,
/// tail padding, and all work semantics. This adapter additionally checks the
/// runtime owner, canonical PDA, writable/non-executable role, and stored bump.
pub fn authenticate_general_v2_clear_work_account_v1(
    view: AccountViewV2<'_>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedGeneralV2ClearWorkAccountV1, RetirementAdapterErrorV2> {
    if view.address != canonical_pda.address() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if view.owner != program_id {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if view.is_executable {
        return Err(RetirementAdapterErrorV2::ExecutableAccount);
    }
    if !view.is_writable {
        return Err(RetirementAdapterErrorV2::NotWritable);
    }
    let work = ClearWorkHeaderV2::decode_account(view.data)?;
    if work.stored_bump != canonical_pda.bump() {
        return Err(RetirementAdapterErrorV2::WrongBump);
    }
    let epoch_generation = work.epoch_generation;
    if epoch_generation == 0 {
        return Err(RetirementErrorV2::WrongGeneration.into());
    }
    Ok(AuthenticatedGeneralV2ClearWorkAccountV1 {
        account: view.address,
        parent: GeneralV2EpochChildParentV1 {
            program_id,
            market: identity(work.market)?,
            epoch_account: identity(work.epoch)?,
            epoch_generation,
        },
        node: identity(work.node)?,
        order_set: identity(work.order_set)?,
        feed: identity(work.feed)?,
        settlement_candidate: identity(work.settlement_candidate_id)?,
        rent: retirement_rent(work.rent)?,
        reward_remaining: work.reward_remaining,
        reward_earned: work.reward_earned,
        phase: work.phase,
    })
}

/// Owner-issued terminal CandidateIndex-page disposition.
///
/// No constructor is exposed: the current CandidateIndex codec does not carry
/// the fresh General V2 Epoch PDA, generation, or deletable-rent owner. Its
/// eventual owner must prove the full active mask closed and bind the exact
/// reverse page cursor before minting this capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedCandidateIndexPageTerminalV1 {
    parent: GeneralV2EpochChildParentV1,
    account: Identity32V1,
    page_index: u16,
    active_count: u16,
    closed_mask: u64,
    close_reward_lamports: u64,
    rent: DeletableRentOwnerV1,
}

impl FamilyOwnedCandidateIndexPageTerminalV1 {
    /// Fresh parent binding owned by the future counted codec.
    pub const fn parent(self) -> GeneralV2EpochChildParentV1 {
        self.parent
    }

    /// Canonical counted page account.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Canonical page index in its exhaustive set.
    pub const fn page_index(self) -> u16 {
        self.page_index
    }

    /// Number of active entries whose complete mask was authenticated.
    pub const fn active_count(self) -> u16 {
        self.active_count
    }

    /// Exact authenticated closed-entry mask.
    pub const fn closed_mask(self) -> u64 {
        self.closed_mask
    }

    /// Prepaid permissionless page-close reward.
    pub const fn close_reward_lamports(self) -> u64 {
        self.close_reward_lamports
    }

    /// Deletable page rent owner; never a reward source.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

/// Owner-issued terminal CandidateVerdict disposition.
///
/// No constructor is exposed until a counted verdict codec binds the fresh
/// parent generation and the candidate owner proves every downstream verdict
/// dependency exhausted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedCandidateVerdictTerminalV1 {
    parent: GeneralV2EpochChildParentV1,
    account: Identity32V1,
    candidate: Identity32V1,
    verdict: Identity32V1,
    rent: DeletableRentOwnerV1,
}

impl FamilyOwnedCandidateVerdictTerminalV1 {
    /// Fresh General V2 parent binding.
    pub const fn parent(self) -> GeneralV2EpochChildParentV1 {
        self.parent
    }

    /// Canonical counted verdict account.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Candidate whose dependency graph was exhausted.
    pub const fn candidate(self) -> Identity32V1 {
        self.candidate
    }

    /// Immutable verdict identity.
    pub const fn verdict(self) -> Identity32V1 {
        self.verdict
    }

    /// Deletable verdict rent owner.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

/// Owner-issued terminal CandidateEscrow disposition.
///
/// Work, bond, cleanup, and solver amounts are retained separately so a future
/// bridge cannot hide an unpaid compartment behind a scalar "terminal" bit.
/// No constructor exists until the counted escrow owner proves every remaining
/// amount zero and every required refund/slash/reward/claim exactly consumed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedCandidateEscrowTerminalV1 {
    parent: GeneralV2EpochChildParentV1,
    account: Identity32V1,
    candidate: Identity32V1,
    payer: Identity32V1,
    refund_destination: Identity32V1,
    work_remaining: u64,
    bond_remaining: u64,
    cleanup_remaining: u64,
    solver_remaining: u64,
    surplus_routed_lamports: u64,
    rent: DeletableRentOwnerV1,
}

impl FamilyOwnedCandidateEscrowTerminalV1 {
    /// Fresh General V2 parent binding.
    pub const fn parent(self) -> GeneralV2EpochChildParentV1 {
        self.parent
    }

    /// Canonical counted escrow account.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Candidate whose economic compartments were exhausted.
    pub const fn candidate(self) -> Identity32V1 {
        self.candidate
    }

    /// Original escrow payer.
    pub const fn payer(self) -> Identity32V1 {
        self.payer
    }

    /// Immutable non-sink refund destination.
    pub const fn refund_destination(self) -> Identity32V1 {
        self.refund_destination
    }

    /// Unpaid work compartment, required to be zero by the future owner.
    pub const fn work_remaining(self) -> u64 {
        self.work_remaining
    }

    /// Unresolved bond compartment, required to be zero by the future owner.
    pub const fn bond_remaining(self) -> u64 {
        self.bond_remaining
    }

    /// Unpaid cleanup compartment, required to be zero by the future owner.
    pub const fn cleanup_remaining(self) -> u64 {
        self.cleanup_remaining
    }

    /// Unclaimed solver compartment, required to be zero by the future owner.
    pub const fn solver_remaining(self) -> u64 {
        self.solver_remaining
    }

    /// Donation-only surplus already routed by authoritative transitions.
    pub const fn surplus_routed_lamports(self) -> u64 {
        self.surplus_routed_lamports
    }

    /// Deletable escrow rent owner, separate from all economic compartments.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

/// Owner-issued terminal ClearWork disposition.
///
/// Exact runtime authentication is already available through
/// [`authenticate_general_v2_clear_work_account_v1`], but this terminal type
/// remains unmintable until `clutch-general-v2-contract` owns the close-reward,
/// remaining-reward, dependent-account, and deletion disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedClearWorkTerminalV1 {
    account: AuthenticatedGeneralV2ClearWorkAccountV1,
    close_reward_lamports: u64,
    refundable_reward_lamports: u64,
}

impl FamilyOwnedClearWorkTerminalV1 {
    /// Exact runtime-authenticated work account.
    pub const fn account(self) -> AuthenticatedGeneralV2ClearWorkAccountV1 {
        self.account
    }

    /// Prepaid permissionless work-close reward.
    pub const fn close_reward_lamports(self) -> u64 {
        self.close_reward_lamports
    }

    /// Any owner-authorized unused reward refund, never donation revenue.
    pub const fn refundable_reward_lamports(self) -> u64 {
        self.refundable_reward_lamports
    }
}

/// Owner-issued terminal counted OrderPage disposition.
///
/// The current OrderPage V4 codec uses the legacy Epoch identity and has no
/// parent generation or deletable-rent owner. Consequently this successor V5
/// capability has no constructor until its authoritative codec and
/// economically-empty page predicate exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedOrderPageTerminalV1 {
    parent: GeneralV2EpochChildParentV1,
    account: Identity32V1,
    page_index: u16,
    page_count: u16,
    live_order_count: u16,
    rent: DeletableRentOwnerV1,
}

impl FamilyOwnedOrderPageTerminalV1 {
    /// Fresh General V2 parent binding.
    pub const fn parent(self) -> GeneralV2EpochChildParentV1 {
        self.parent
    }

    /// Canonical counted OrderPage account.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Canonical page index in the frozen page set.
    pub const fn page_index(self) -> u16 {
        self.page_index
    }

    /// Exact frozen page-set width.
    pub const fn page_count(self) -> u16 {
        self.page_count
    }

    /// Authenticated live-order count, required to be zero by the owner.
    pub const fn live_order_count(self) -> u16 {
        self.live_order_count
    }

    /// Deletable page rent owner.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

/// Owner-issued terminal counted Reservation-archive disposition.
///
/// The current General Reservation V7 envelope is intentionally not accepted:
/// its body carries the legacy semantic Epoch identity. The eventual General
/// V2 successor must bind the fresh Epoch PDA/generation and prove released or
/// fully consumed state, zero remaining assets, paid quantity/cash agreement,
/// and an already-cleared Position count marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedReservationArchiveTerminalV1 {
    parent: GeneralV2EpochChildParentV1,
    account: Identity32V1,
    position: Identity32V1,
    position_generation: u64,
    state: u8,
    remaining_cash_atoms: u64,
    remaining_internal_atoms: [u64; MAX_OUTCOMES],
    entitled_units: u64,
    consumed_units: u64,
    paid_units: u64,
    position_counted: bool,
    rent: DeletableRentOwnerV1,
}

impl FamilyOwnedReservationArchiveTerminalV1 {
    /// Fresh General V2 parent binding.
    pub const fn parent(self) -> GeneralV2EpochChildParentV1 {
        self.parent
    }

    /// Canonical counted Reservation account.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Position whose assets and count were released.
    pub const fn position(self) -> Identity32V1 {
        self.position
    }

    /// Generation of the bound Position.
    pub const fn position_generation(self) -> u64 {
        self.position_generation
    }

    /// Authoritative reservation terminal state.
    pub const fn state(self) -> u8 {
        self.state
    }

    /// Remaining reservation-owned cash, required to be zero.
    pub const fn remaining_cash_atoms(self) -> u64 {
        self.remaining_cash_atoms
    }

    /// Remaining reservation-owned claim atoms, required to be all zero.
    pub const fn remaining_internal_atoms(self) -> [u64; MAX_OUTCOMES] {
        self.remaining_internal_atoms
    }

    /// Exact stamped entitlement quantity.
    pub const fn entitled_units(self) -> u64 {
        self.entitled_units
    }

    /// Exact consumed claim quantity.
    pub const fn consumed_units(self) -> u64 {
        self.consumed_units
    }

    /// Exact cash-settled quantity; terminal consumed state requires equality.
    pub const fn paid_units(self) -> u64 {
        self.paid_units
    }

    /// Whether the account still owns a Position outstanding-count unit.
    pub const fn position_counted(self) -> bool {
        self.position_counted
    }

    /// Deletable Reservation rent owner.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

/// Owner-issued terminal counted SettlementReceipt disposition.
///
/// This preserves independent real-end consumption, virtual-end shape, slice
/// exhaustion, and owner-row dependency counts. The legacy receipt V2 body
/// lacks a fresh General V2 parent generation, so no constructor is exposed
/// until Receipt V3 and the owner-settlement terminal join both exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedSettlementReceiptTerminalV1 {
    parent: GeneralV2EpochChildParentV1,
    account: Identity32V1,
    candidate: Identity32V1,
    sequence: u64,
    slice_index: u16,
    expected_end_mask: u8,
    consumed_end_mask: u8,
    quantity: u64,
    settled_quantity: u64,
    consideration_price_units: u128,
    owner_rows_remaining: u16,
    rent: DeletableRentOwnerV1,
}

impl FamilyOwnedSettlementReceiptTerminalV1 {
    /// Fresh General V2 parent binding.
    pub const fn parent(self) -> GeneralV2EpochChildParentV1 {
        self.parent
    }

    /// Canonical counted receipt account.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Selected candidate whose exact slice was settled.
    pub const fn candidate(self) -> Identity32V1 {
        self.candidate
    }

    /// Canonical monotonically increasing receipt sequence.
    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    /// Canonical zero-based selected-witness slice index.
    pub const fn slice_index(self) -> u16 {
        self.slice_index
    }

    /// Real ends present on the receipt; virtual ends have no bit.
    pub const fn expected_end_mask(self) -> u8 {
        self.expected_end_mask
    }

    /// Independently consumed real ends.
    pub const fn consumed_end_mask(self) -> u8 {
        self.consumed_end_mask
    }

    /// Exact slice quantity.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }

    /// Exact settled quantity, required to equal `quantity` at retirement.
    pub const fn settled_quantity(self) -> u64 {
        self.settled_quantity
    }

    /// Frozen exact consideration in price units.
    pub const fn consideration_price_units(self) -> u128 {
        self.consideration_price_units
    }

    /// Owner-settlement rows still depending on this receipt, required zero.
    pub const fn owner_rows_remaining(self) -> u16 {
        self.owner_rows_remaining
    }

    /// Deletable receipt rent owner.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }
}

/// Exact liability compartments retained by a General V2 FinalPot terminal
/// authority.
///
/// These values are collateral accounting, not lamport donations. Rounding
/// slack remains in exact price units; virtual claims and their cash remain
/// distinct; selected fees remain bound to their selected fee record. No
/// field may be routed to the neutral sink. Only unsolicited lamports above
/// the separately stored refundable rent principal are donation-eligible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralV2FinalPotLiabilityCompartmentsV1 {
    price_scale: u64,
    owner_order_set_digest: [u8; 32],
    owner_count: u16,
    finalized_owner_count: u16,
    consideration_debit_atoms: u64,
    seller_credit_atoms: u64,
    available_consideration_atoms: u64,
    rounding_slack_price_units: u128,
    realized_rounding_price_units: u128,
    virtual_claim_cash_atoms: u64,
    virtual_claim_internal_atoms: [u64; MAX_OUTCOMES],
    fee_record: [u8; 32],
    selected_fee_atoms: u64,
    collected_fee_atoms: u64,
    retired_owner_count: u16,
    rounding_authority: [u8; 32],
    virtual_claim_authority: [u8; 32],
    fee_authority: [u8; 32],
    rounding_disposition_id: [u8; 32],
    virtual_claim_disposition_id: [u8; 32],
    fee_disposition_id: [u8; 32],
}

impl GeneralV2FinalPotLiabilityCompartmentsV1 {
    /// Exact collateral price scale.
    pub const fn price_scale(self) -> u64 {
        self.price_scale
    }

    /// Digest of the exact frozen owner/order membership book.
    pub const fn owner_order_set_digest(self) -> [u8; 32] {
        self.owner_order_set_digest
    }

    /// Exact frozen owner-row count.
    pub const fn owner_count(self) -> u16 {
        self.owner_count
    }

    /// Exactly finalized owner rows; terminality requires full equality.
    pub const fn finalized_owner_count(self) -> u16 {
        self.finalized_owner_count
    }

    /// Sum of owner payer conversions, excluding selected fees.
    pub const fn consideration_debit_atoms(self) -> u64 {
        self.consideration_debit_atoms
    }

    /// Sum of owner payee conversions.
    pub const fn seller_credit_atoms(self) -> u64 {
        self.seller_credit_atoms
    }

    /// Buyer consideration still held under explicit liability ownership.
    pub const fn available_consideration_atoms(self) -> u64 {
        self.available_consideration_atoms
    }

    /// Relation-certified rounding slack in exact price units.
    pub const fn rounding_slack_price_units(self) -> u128 {
        self.rounding_slack_price_units
    }

    /// Rounding slack actually realized by finalized owner rows.
    pub const fn realized_rounding_price_units(self) -> u128 {
        self.realized_rounding_price_units
    }

    /// Whole collateral atoms owned by the terminal virtual-claim cash ledger.
    pub const fn virtual_claim_cash_atoms(self) -> u64 {
        self.virtual_claim_cash_atoms
    }

    /// Per-outcome claim principal owned by the virtual pot position.
    pub const fn virtual_claim_internal_atoms(self) -> [u64; MAX_OUTCOMES] {
        self.virtual_claim_internal_atoms
    }

    /// Selected fee-record bytes; zero only when both fee amounts are zero.
    pub const fn fee_record(self) -> [u8; 32] {
        self.fee_record
    }

    /// Exact selected fee liability in collateral atoms.
    pub const fn selected_fee_atoms(self) -> u64 {
        self.selected_fee_atoms
    }

    /// Exact fees already collected under `fee_record`.
    pub const fn collected_fee_atoms(self) -> u64 {
        self.collected_fee_atoms
    }

    /// Owner rows atomically retired into the FinalPot latch.
    pub const fn retired_owner_count(self) -> u16 {
        self.retired_owner_count
    }

    /// Semantic authority for rounding-slack disposition.
    pub const fn rounding_authority(self) -> [u8; 32] {
        self.rounding_authority
    }

    /// Semantic authority for virtual claim/cash disposition.
    pub const fn virtual_claim_authority(self) -> [u8; 32] {
        self.virtual_claim_authority
    }

    /// Fee authority bytes; zero exactly for the zero-fee case.
    pub const fn fee_authority(self) -> [u8; 32] {
        self.fee_authority
    }

    /// Once-only rounding disposition receipt.
    pub const fn rounding_disposition_id(self) -> [u8; 32] {
        self.rounding_disposition_id
    }

    /// Once-only virtual-claim disposition receipt.
    pub const fn virtual_claim_disposition_id(self) -> [u8; 32] {
        self.virtual_claim_disposition_id
    }

    /// Once-only selected-fee disposition receipt.
    pub const fn fee_disposition_id(self) -> [u8; 32] {
        self.fee_disposition_id
    }
}

/// Owner-issued terminal counted FinalPot disposition.
///
/// No constructor is exposed. The current legacy FinalPot V2 "closed means
/// all zero" predicate is insufficient and is intentionally not reused. The
/// future owner must join the complete owner-settlement book and independently
/// authorize every liability compartment before this capability can exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedFinalPotTerminalV1 {
    parent: GeneralV2EpochChildParentV1,
    account: Identity32V1,
    candidate: Identity32V1,
    liabilities: GeneralV2FinalPotLiabilityCompartmentsV1,
    rent: DeletableRentOwnerV1,
}

/// Lamport-only FinalPot close split that cannot name any collateral
/// liability as neutral-sink revenue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalPotDonationOnlyLamportDispositionV1 {
    rent_payer: Identity32V1,
    rent_refund_lamports: u64,
    neutral_sink: Identity32V1,
    donation_lamports: u64,
}

impl FinalPotDonationOnlyLamportDispositionV1 {
    /// Stored payer receiving only the refundable rent principal.
    pub const fn rent_payer(self) -> Identity32V1 {
        self.rent_payer
    }

    /// Exact refundable rent principal.
    pub const fn rent_refund_lamports(self) -> u64 {
        self.rent_refund_lamports
    }

    /// Immutable donation sink, never a collateral-liability recipient.
    pub const fn neutral_sink(self) -> Identity32V1 {
        self.neutral_sink
    }

    /// Hostile prefund plus later unsolicited account lamports only.
    pub const fn donation_lamports(self) -> u64 {
        self.donation_lamports
    }
}

impl FamilyOwnedFinalPotTerminalV1 {
    /// Fresh General V2 parent binding.
    pub const fn parent(self) -> GeneralV2EpochChildParentV1 {
        self.parent
    }

    /// Canonical counted FinalPot account.
    pub const fn account(self) -> Identity32V1 {
        self.account
    }

    /// Selected candidate owning the pot ledger.
    pub const fn candidate(self) -> Identity32V1 {
        self.candidate
    }

    /// Exact separately owned collateral liability compartments.
    pub const fn liabilities(self) -> GeneralV2FinalPotLiabilityCompartmentsV1 {
        self.liabilities
    }

    /// Deletable account rent owner; only surplus account lamports are
    /// donation-eligible after returning this principal.
    pub const fn rent(self) -> DeletableRentOwnerV1 {
        self.rent
    }

    /// Split only the Solana account's lamports after every separately named
    /// collateral liability disposition has already been authorized in the
    /// same atomic plan.
    ///
    /// The neutral sink receives exactly the balance above refundable rent
    /// principal. Rounding price units, virtual claims/cash, and selected fees
    /// are not inputs to this arithmetic and therefore cannot be neutralized.
    /// The sink must come from the exact same General V2 Market binding.
    pub fn plan_donation_only_lamport_close(
        self,
        actual_account_lamports: u64,
        neutral_sink: AuthenticatedGeneralV2NeutralSinkBindingV1,
    ) -> Result<FinalPotDonationOnlyLamportDispositionV1, RetirementAdapterErrorV2> {
        self.rent.validate()?;
        if neutral_sink.program_id != self.parent.program_id
            || neutral_sink.market != self.parent.market
        {
            return Err(RetirementErrorV2::WrongParent.into());
        }
        if self.account == neutral_sink.account || self.account == neutral_sink.neutral_sink {
            return Err(RetirementErrorV2::AccountAlias.into());
        }
        if self.rent.payer() == neutral_sink.neutral_sink {
            return Err(RetirementErrorV2::PayerIsNeutralSink.into());
        }
        let required_floor = self
            .rent
            .refundable_principal()
            .checked_add(self.rent.donation_floor())
            .ok_or(RetirementErrorV2::ArithmeticOverflow)?;
        if actual_account_lamports < required_floor {
            return Err(RetirementErrorV2::AccountBalanceShortfall.into());
        }
        Ok(FinalPotDonationOnlyLamportDispositionV1 {
            rent_payer: self.rent.payer(),
            rent_refund_lamports: self.rent.refundable_principal(),
            neutral_sink: neutral_sink.neutral_sink,
            donation_lamports: actual_account_lamports
                .checked_sub(self.rent.refundable_principal())
                .ok_or(RetirementErrorV2::AccountBalanceShortfall)?,
        })
    }
}

/// Exact runtime-authenticated, semantic-owner-issued FinalPot terminal proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedGeneralV2FinalPotTerminalV1 {
    terminal: FamilyOwnedFinalPotTerminalV1,
    family: AuthenticatedEpochChildFamilyV2,
}

impl AuthenticatedGeneralV2FinalPotTerminalV1 {
    /// Complete explicit-liability FinalPot terminal facts.
    pub const fn terminal(self) -> FamilyOwnedFinalPotTerminalV1 {
        self.terminal
    }

    /// Exact family evidence consumable by the prospective zero-count root join.
    pub const fn family(self) -> AuthenticatedEpochChildFamilyV2 {
        self.family
    }
}

/// Authenticate the reserved-disabled `0x89/1` FinalPot and mint terminal
/// evidence only from its semantic owner's exhaustive disposition.
pub fn authenticate_general_v2_final_pot_terminal_v1(
    view: AccountViewV2<'_>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedGeneralV2FinalPotTerminalV1, RetirementAdapterErrorV2> {
    let authenticated = authenticate_exact_family_account_v1(
        view,
        program_id,
        canonical_pda,
        AccountAccessV2::Writable,
        ExactFamilySchemaV1 {
            tag: FINAL_POT_ACCOUNT_TAG,
            version: FINAL_POT_ACCOUNT_VERSION,
            len: FINAL_POT_ACCOUNT_BYTES,
            stored_bump_offset: FINAL_POT_ACCOUNT_BYTES - 2,
        },
    )?;
    let account = GeneralV2FinalPotV1AccountV1::decode(authenticated.data)?;
    let disposition = account
        .semantic
        .retirement_disposition()
        .map_err(|_| RetirementAdapterErrorV2::InvalidSchema)?;
    let parent = GeneralV2EpochChildParentV1 {
        program_id,
        market: Identity32V1::new(disposition.market())?,
        epoch_account: Identity32V1::new(disposition.epoch())?,
        epoch_generation: disposition.epoch_generation(),
    };
    let candidate = Identity32V1::new(disposition.candidate())?;
    for independent in [parent.market, parent.epoch_account, candidate] {
        if authenticated.address == independent {
            return Err(RetirementErrorV2::AccountAlias.into());
        }
    }
    let semantic = account.semantic;
    let expected = semantic.settled.expectation;
    let terminal = FamilyOwnedFinalPotTerminalV1 {
        parent,
        account: authenticated.address,
        candidate,
        liabilities: GeneralV2FinalPotLiabilityCompartmentsV1 {
            price_scale: expected.price_scale,
            owner_order_set_digest: expected.owner_order_set_digest,
            owner_count: expected.owner_count,
            finalized_owner_count: semantic.settled.finalized_owner_count,
            consideration_debit_atoms: expected.consideration_debit_atoms,
            seller_credit_atoms: expected.seller_credit_atoms,
            available_consideration_atoms: semantic.settled.available_consideration_atoms,
            rounding_slack_price_units: expected.rounding_pot_price_units,
            realized_rounding_price_units: semantic.settled.realized_rounding_price_units,
            virtual_claim_cash_atoms: expected.terminal_claim_cash_atoms,
            virtual_claim_internal_atoms: semantic.initial_virtual_claim_internal_atoms,
            fee_record: expected.fee_record,
            selected_fee_atoms: expected.selected_fee_atoms,
            collected_fee_atoms: semantic.settled.collected_fee_atoms,
            retired_owner_count: semantic.retired_owner_count,
            rounding_authority: semantic.authorities.rounding_authority,
            virtual_claim_authority: semantic.authorities.virtual_claim_authority,
            fee_authority: semantic.authorities.fee_authority,
            rounding_disposition_id: disposition.rounding_disposition_id(),
            virtual_claim_disposition_id: disposition.virtual_claim_disposition_id(),
            fee_disposition_id: disposition.fee_disposition_id(),
        },
        rent: retirement_rent(account.rent)?,
    };
    Ok(AuthenticatedGeneralV2FinalPotTerminalV1 {
        terminal,
        family: AuthenticatedEpochChildFamilyV2 {
            kind: GeneralV2EpochChildKindV1::FinalPot,
            program_id,
            market: parent.market,
            epoch_account: parent.epoch_account,
            epoch_generation: parent.epoch_generation,
            runtime: FamilyRuntimeEvidenceV1::ExactTerminalAccount(authenticated.address),
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FamilyRuntimeEvidenceV1 {
    ExactTerminalAccount(Identity32V1),
    CanonicalAbsence(Identity32V1),
}

/// Narrow semantic-owner input for one terminal family authority account.
///
/// Its fields intentionally have no public constructor.  A future
/// family-specific bridge must decode its authoritative codec, prove the
/// complete family's terminal economics/enumeration, and only then mint this
/// value in this module.  Even then it cannot authorize root close without the
/// exact runtime authentication performed by
/// [`authenticate_epoch_child_terminal_account_v2`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedTerminalEpochChildV1 {
    kind: GeneralV2EpochChildKindV1,
    program_id: Identity32V1,
    market: Identity32V1,
    epoch_account: Identity32V1,
    epoch_generation: u64,
    schema: ExactFamilySchemaV1,
    access: AccountAccessV2,
}

/// Narrow semantic-owner input proving an exhaustively enumerated family has
/// exactly one remaining canonical address and that address must be absent.
///
/// This is not a general claim that one absent PDA proves an arbitrary family
/// empty.  Its fields intentionally have no public constructor.  A future
/// family owner may mint it only after proving its bounded enumeration leaves
/// this single canonical absence check as the final runtime obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FamilyOwnedFinalAbsenceEpochChildV1 {
    kind: GeneralV2EpochChildKindV1,
    program_id: Identity32V1,
    market: Identity32V1,
    epoch_account: Identity32V1,
    epoch_generation: u64,
}

/// Adapter-issued evidence that one semantic owner exhausted one complete
/// counted child family for one exact Epoch generation.
///
/// There is intentionally no public constructor.  A family bridge must first
/// authenticate the exact runtime account or canonical absence and then invoke
/// the owning codec's terminal/absence contract.  This type alone is not root
/// close authority and cannot be obtained from a client projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedEpochChildFamilyV2 {
    kind: GeneralV2EpochChildKindV1,
    program_id: Identity32V1,
    market: Identity32V1,
    epoch_account: Identity32V1,
    epoch_generation: u64,
    runtime: FamilyRuntimeEvidenceV1,
}

impl AuthenticatedEpochChildFamilyV2 {
    /// Frozen Epoch child class authenticated by the family owner.
    pub const fn kind(self) -> GeneralV2EpochChildKindV1 {
        self.kind
    }

    /// Runtime account or canonical absent address checked by the adapter.
    pub const fn evidence_address(self) -> Identity32V1 {
        match self.runtime {
            FamilyRuntimeEvidenceV1::ExactTerminalAccount(address)
            | FamilyRuntimeEvidenceV1::CanonicalAbsence(address) => address,
        }
    }

    fn require(
        self,
        kind: GeneralV2EpochChildKindV1,
        epoch: AuthenticatedGeneralV2TerminalEpochV1,
    ) -> Result<(), RetirementAdapterErrorV2> {
        let parent = epoch.parent;
        if self.kind != kind {
            return Err(RetirementErrorV2::WrongChildKind.into());
        }
        if self.program_id != parent.program_id
            || self.market != parent.market
            || self.epoch_account != parent.epoch_account
        {
            return Err(RetirementErrorV2::WrongParent.into());
        }
        if self.epoch_generation != parent.epoch_generation {
            return Err(RetirementErrorV2::WrongGeneration.into());
        }
        if epoch.children.get(kind) != 0 {
            return Err(RetirementErrorV2::ChildOutstanding.into());
        }
        Ok(())
    }
}

/// Combine a family-owned terminality capability with exact runtime account
/// authentication.
///
/// Owner, canonical PDA, exact length/header, stored bump, access role, and
/// non-executable state are all checked here.  The opaque semantic input is
/// intentionally insufficient by itself and no family currently exposes a
/// public raw constructor for it.
pub fn authenticate_epoch_child_terminal_account_v2(
    account: AccountViewV2<'_>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    semantic: FamilyOwnedTerminalEpochChildV1,
) -> Result<AuthenticatedEpochChildFamilyV2, RetirementAdapterErrorV2> {
    if semantic.epoch_generation == 0 {
        return Err(RetirementErrorV2::WrongGeneration.into());
    }
    if semantic.program_id != program_id {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    let authenticated = authenticate_exact_family_account_v1(
        account,
        program_id,
        canonical_pda,
        semantic.access,
        semantic.schema,
    )?;
    Ok(AuthenticatedEpochChildFamilyV2 {
        kind: semantic.kind,
        program_id,
        market: semantic.market,
        epoch_account: semantic.epoch_account,
        epoch_generation: semantic.epoch_generation,
        runtime: FamilyRuntimeEvidenceV1::ExactTerminalAccount(authenticated.address),
    })
}

/// Combine an exhaustive family-owned final-absence capability with exact
/// canonical System-owned absence authentication.
pub fn authenticate_epoch_child_final_absence_v2(
    absence: AbsentAccountViewV1,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    semantic: FamilyOwnedFinalAbsenceEpochChildV1,
) -> Result<AuthenticatedEpochChildFamilyV2, RetirementAdapterErrorV2> {
    if semantic.epoch_generation == 0 {
        return Err(RetirementErrorV2::WrongGeneration.into());
    }
    if semantic.program_id != program_id {
        return Err(RetirementAdapterErrorV2::WrongOwner);
    }
    if absence.address != canonical_pda.address() {
        return Err(RetirementAdapterErrorV2::WrongPda);
    }
    if absence.is_writable {
        return Err(RetirementAdapterErrorV2::UnexpectedWritable);
    }
    if absence.is_executable || absence.owner != [0; 32] || absence.data_len != 0 {
        return Err(RetirementAdapterErrorV2::AccountNotAbsent);
    }
    Ok(AuthenticatedEpochChildFamilyV2 {
        kind: semantic.kind,
        program_id,
        market: semantic.market,
        epoch_account: semantic.epoch_account,
        epoch_generation: semantic.epoch_generation,
        runtime: FamilyRuntimeEvidenceV1::CanonicalAbsence(absence.address),
    })
}

/// Structurally exhaustive set of all nine independently counted child
/// families.  Named fields prevent a caller from truncating the set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedEpochChildFamiliesV2 {
    /// Admission-node/candidate bundle family.
    pub candidate_bundle: AuthenticatedEpochChildFamilyV2,
    /// CandidateIndex page family.
    pub candidate_index_page: AuthenticatedEpochChildFamilyV2,
    /// Immutable candidate-verdict family.
    pub candidate_verdict: AuthenticatedEpochChildFamilyV2,
    /// Candidate economic-escrow family.
    pub candidate_escrow: AuthenticatedEpochChildFamilyV2,
    /// Active-width ClearWork family.
    pub clear_work_bundle: AuthenticatedEpochChildFamilyV2,
    /// Counted OrderPage family.
    pub order_page: AuthenticatedEpochChildFamilyV2,
    /// Counted Reservation archive family.
    pub reservation_archive: AuthenticatedEpochChildFamilyV2,
    /// Counted SettlementReceipt family.
    pub settlement_receipt: AuthenticatedEpochChildFamilyV2,
    /// Unique FinalPot family.
    pub final_pot: AuthenticatedEpochChildFamilyV2,
}

impl AuthenticatedEpochChildFamiliesV2 {
    fn array(
        self,
    ) -> [(GeneralV2EpochChildKindV1, AuthenticatedEpochChildFamilyV2); EPOCH_CHILD_CLASS_CAPACITY_V2] {
        [
            (GeneralV2EpochChildKindV1::CandidateBundle, self.candidate_bundle),
            (
                GeneralV2EpochChildKindV1::CandidateIndexPage,
                self.candidate_index_page,
            ),
            (GeneralV2EpochChildKindV1::CandidateVerdict, self.candidate_verdict),
            (GeneralV2EpochChildKindV1::CandidateEscrow, self.candidate_escrow),
            (GeneralV2EpochChildKindV1::ClearWorkBundle, self.clear_work_bundle),
            (GeneralV2EpochChildKindV1::OrderPage, self.order_page),
            (
                GeneralV2EpochChildKindV1::ReservationArchive,
                self.reservation_archive,
            ),
            (GeneralV2EpochChildKindV1::SettlementReceipt, self.settlement_receipt),
            (GeneralV2EpochChildKindV1::FinalPot, self.final_pot),
        ]
    }
}

/// Private-field result of joining an exact fresh General V2 terminal Epoch
/// with all nine family-owned terminal-or-absent capabilities.
///
/// This remains a prerequisite rather than root-close authority: it does not
/// include the authenticated Window/Budget deletion plans, immutable neutral
/// sink, recipient accounts, or runtime rollback contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedTerminalEpochFamiliesV2 {
    epoch: AuthenticatedGeneralV2TerminalEpochV1,
    evidence_addresses: [Identity32V1; EPOCH_CHILD_CLASS_CAPACITY_V2],
}

impl AuthenticatedTerminalEpochFamiliesV2 {
    /// Canonical fresh General V2 terminal Epoch account address.
    pub const fn epoch_account(self) -> Identity32V1 {
        self.epoch.parent.epoch_account
    }

    /// Exact private-field terminal Epoch capability bound to every family.
    pub const fn epoch(self) -> AuthenticatedGeneralV2TerminalEpochV1 {
        self.epoch
    }

    /// Runtime authority/absence address for each frozen class order.
    pub const fn evidence_addresses(self) -> [Identity32V1; EPOCH_CHILD_CLASS_CAPACITY_V2] {
        self.evidence_addresses
    }
}

/// Join the exact fresh General V2 terminal Epoch with all nine family proofs.
///
/// A legacy Epoch V5 account or semantic id cannot enter this join.
pub fn authenticate_terminal_epoch_families_v2(
    authenticated_epoch: AuthenticatedGeneralV2TerminalEpochV1,
    families: AuthenticatedEpochChildFamiliesV2,
) -> Result<AuthenticatedTerminalEpochFamiliesV2, RetirementAdapterErrorV2> {
    if !authenticated_epoch.children.is_zero() {
        return Err(RetirementErrorV2::ChildOutstanding.into());
    }

    let entries = families.array();
    let mut evidence_addresses =
        [authenticated_epoch.parent.epoch_account; EPOCH_CHILD_CLASS_CAPACITY_V2];
    let mut index = 0usize;
    while index < entries.len() {
        let (expected_kind, family) = entries[index];
        family.require(expected_kind, authenticated_epoch)?;
        evidence_addresses[index] = family.evidence_address();
        index += 1;
    }

    Ok(AuthenticatedTerminalEpochFamiliesV2 {
        epoch: authenticated_epoch,
        evidence_addresses,
    })
}

const _: () = assert!(WINDOW_STORED_BUMP_OFFSET < WINDOW_ACCOUNT_BYTES);
const _: () = assert!(MARKET_BINDING_STORED_BUMP_OFFSET < MARKET_BINDING_ACCOUNT_BYTES);
const _: () = assert!(EPOCH_CHILD_CLASS_CAPACITY_V2 == 9);
