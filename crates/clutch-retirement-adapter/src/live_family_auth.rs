// SPDX-License-Identifier: AGPL-3.0-or-later

//! Production-bound authentication for general-Epoch retirement families.
//!
//! This module deliberately has no generic public "attestation" constructor.
//! A class witness can be minted only inside this adapter after both an exact
//! runtime account/absence check and the owning family's semantic terminality
//! check.  Today the General V2 Window and immutable Market binding have both
//! halves.  The remaining child families stay unmintable until their semantic
//! owners expose equivalent terminal-or-absent capabilities.

use clutch_general_v2_contract::{
    CandidateWindowV4AccountV1, MarketBindingV1, MARKET_BINDING_ACCOUNT_BYTES,
    MARKET_BINDING_ACCOUNT_TAG, MARKET_BINDING_ACCOUNT_VERSION, WINDOW_ACCOUNT_BYTES,
    WINDOW_ACCOUNT_TAG, WINDOW_ACCOUNT_VERSION,
};
use clutch_retirement::{
    DeletableRentOwnerV1, EpochChildKindV1, GeneralEpochPhaseV2, Identity32V1,
    LiveGeneralEpochProjectionV2, RetirementErrorV2,
};

use crate::account_auth::{
    authenticate_epoch_budget_v1_exact, AbsentAccountViewV1, AccountAccessV2, AccountViewV2,
    AuthenticatedAccountV2, CanonicalPdaV1,
};
use crate::composition::{project_live_general_epoch_retirement_v2, GeneralEpochAccountV5};
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

fn terminal_epoch(
    authenticated_epoch: AuthenticatedAccountV2<'_>,
) -> Result<LiveGeneralEpochProjectionV2, RetirementAdapterErrorV2> {
    let epoch = project_live_general_epoch_retirement_v2(GeneralEpochAccountV5::decode(
        authenticated_epoch.data(),
    )?)?;
    if !matches!(
        epoch.phase,
        GeneralEpochPhaseV2::Settled | GeneralEpochPhaseV2::Lapsed
    ) {
        return Err(RetirementErrorV2::WrongPhase.into());
    }
    Ok(epoch)
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
    kind: EpochChildKindV1,
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
    kind: EpochChildKindV1,
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
    kind: EpochChildKindV1,
    market: Identity32V1,
    epoch_account: Identity32V1,
    epoch_generation: u64,
    runtime: FamilyRuntimeEvidenceV1,
}

impl AuthenticatedEpochChildFamilyV2 {
    /// Frozen Epoch child class authenticated by the family owner.
    pub const fn kind(self) -> EpochChildKindV1 {
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
        kind: EpochChildKindV1,
        epoch: LiveGeneralEpochProjectionV2,
        epoch_account: Identity32V1,
    ) -> Result<(), RetirementAdapterErrorV2> {
        if self.kind != kind {
            return Err(RetirementErrorV2::WrongChildKind.into());
        }
        if self.market != epoch.market || self.epoch_account != epoch_account {
            return Err(RetirementErrorV2::WrongParent.into());
        }
        if self.epoch_generation != epoch.retirement.epoch_generation {
            return Err(RetirementErrorV2::WrongGeneration.into());
        }
        if epoch.retirement.children.get(kind) != 0 {
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
    let authenticated = authenticate_exact_family_account_v1(
        account,
        program_id,
        canonical_pda,
        semantic.access,
        semantic.schema,
    )?;
    Ok(AuthenticatedEpochChildFamilyV2 {
        kind: semantic.kind,
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
    canonical_pda: CanonicalPdaV1,
    semantic: FamilyOwnedFinalAbsenceEpochChildV1,
) -> Result<AuthenticatedEpochChildFamilyV2, RetirementAdapterErrorV2> {
    if semantic.epoch_generation == 0 {
        return Err(RetirementErrorV2::WrongGeneration.into());
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
    ) -> [(EpochChildKindV1, AuthenticatedEpochChildFamilyV2); EPOCH_CHILD_CLASS_CAPACITY_V2] {
        [
            (EpochChildKindV1::CandidateBundle, self.candidate_bundle),
            (
                EpochChildKindV1::CandidateIndexPage,
                self.candidate_index_page,
            ),
            (EpochChildKindV1::CandidateVerdict, self.candidate_verdict),
            (EpochChildKindV1::CandidateEscrow, self.candidate_escrow),
            (EpochChildKindV1::ClearWorkBundle, self.clear_work_bundle),
            (EpochChildKindV1::OrderPage, self.order_page),
            (
                EpochChildKindV1::ReservationArchive,
                self.reservation_archive,
            ),
            (EpochChildKindV1::SettlementReceipt, self.settlement_receipt),
            (EpochChildKindV1::FinalPot, self.final_pot),
        ]
    }
}

/// Private-field result of joining an exact terminal Epoch V5 with all nine
/// family-owned terminal-or-absent capabilities.
///
/// This remains a prerequisite rather than root-close authority: it does not
/// include the authenticated Window/Budget deletion plans, immutable neutral
/// sink, recipient accounts, or runtime rollback contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedTerminalEpochFamiliesV2 {
    epoch_account: Identity32V1,
    epoch: LiveGeneralEpochProjectionV2,
    evidence_addresses: [Identity32V1; EPOCH_CHILD_CLASS_CAPACITY_V2],
}

impl AuthenticatedTerminalEpochFamiliesV2 {
    /// Canonical terminal Epoch V5 account address.
    pub const fn epoch_account(self) -> Identity32V1 {
        self.epoch_account
    }

    /// Exact terminal Epoch projection bound to every family proof.
    pub const fn epoch(self) -> LiveGeneralEpochProjectionV2 {
        self.epoch
    }

    /// Runtime authority/absence address for each frozen class order.
    pub const fn evidence_addresses(self) -> [Identity32V1; EPOCH_CHILD_CLASS_CAPACITY_V2] {
        self.evidence_addresses
    }
}

/// Join the exact terminal Epoch V5 with all nine family-owned proofs.
pub fn authenticate_terminal_epoch_families_v2(
    authenticated_epoch: AuthenticatedAccountV2<'_>,
    families: AuthenticatedEpochChildFamiliesV2,
) -> Result<AuthenticatedTerminalEpochFamiliesV2, RetirementAdapterErrorV2> {
    let epoch = terminal_epoch(authenticated_epoch)?;
    if !epoch.retirement.children.is_zero() {
        return Err(RetirementErrorV2::ChildOutstanding.into());
    }

    let entries = families.array();
    let mut evidence_addresses = [authenticated_epoch.address(); EPOCH_CHILD_CLASS_CAPACITY_V2];
    let mut index = 0usize;
    while index < entries.len() {
        let (expected_kind, family) = entries[index];
        family.require(expected_kind, epoch, authenticated_epoch.address())?;
        evidence_addresses[index] = family.evidence_address();
        index += 1;
    }

    Ok(AuthenticatedTerminalEpochFamiliesV2 {
        epoch_account: authenticated_epoch.address(),
        epoch,
        evidence_addresses,
    })
}

const _: () = assert!(WINDOW_STORED_BUMP_OFFSET < WINDOW_ACCOUNT_BYTES);
const _: () = assert!(MARKET_BINDING_STORED_BUMP_OFFSET < MARKET_BINDING_ACCOUNT_BYTES);
const _: () = assert!(EPOCH_CHILD_CLASS_CAPACITY_V2 == 9);
