//! General's escrow: where its compartments are named, and what its
//! accounting refers to.
//!
//! Decision 0010 §6 item 3 states one gap and hides a second, and this module
//! closes both.
//!
//! The stated one is movement: "the work escrow is accounted and not yet
//! moved ... nothing yet moves lamports, because a transfer is an account
//! operation and these are pure transitions." True, and it is the smaller half.
//! The hidden one is that nothing binds the accounting to a balance either.
//! [`crate::candidate_v1::GeneralCandidateV1::validate_capitalization`] compares
//! `verification_remaining` against a number derived from the SAME record, so a
//! submission whose account holds nothing at all re-proves its capitalization at
//! every transition and passes every time. A compartment taxonomy that is exact
//! about a quantity with no referent is exact about nothing. §1 and §2 give it a
//! referent: the escrow's own observed balance, re-proven at every transition,
//! with the movement and the accounting constructed together so they cannot be
//! written apart.
//!
//! §3 is a different repair, and it is the one that was load-bearing.
//!
//! # The compartments were stated three times, and one of them disagreed
//!
//! Decision 0010 §2 rules that admission MOVES the maker's worst case, and that
//! `Collect` therefore runs `Settlement(order_id) -> Settlement(candidate_id)`
//! with "the old `External(owner)` route refused outright". That ruling landed
//! in the pure packet builder ([`crate::child_packets`]) and in the batch
//! record ([`crate::collection_v1`]). **It did not land in the artifact.**
//!
//! `effect_artifacts_v3::build_action` built `Collect`'s Custody template as
//! `External -> Settlement`, and `artifacts_v3::validate_routes` required
//! exactly that of any release. `Collect` is not one of the actions whose
//! compartment bytes the admitted EffectProgram patches at runtime -- only
//! `Materialize` is -- so the template's literals are what a chain-executed
//! `Collect` carries. And Custody's own `CustodyRequestV1::validate` makes the
//! two readings mutually exclusive rather than merely different: a `Transfer`
//! must satisfy
//!
//! ```text
//! (source_compartment == External) == is_zero(source_vault_context)
//! (source_compartment == External) == !is_zero(source_owner)
//! ```
//!
//! so an `External` source REQUIRES a nonzero owner and a zero vault context,
//! while `build_row_custody_packets_v2` requires a zero owner and
//! `source_vault_context == order_id`. **A frame either side would accept, the
//! other refuses.** The published artifact still debits the maker's own
//! external account at settlement time -- the live credit regression decision
//! 0009 §2 named and decision 0010 §2 believed it had closed.
//!
//! It survived for the reason its two siblings did. `build_order_escrow_packets_v1`
//! and `build_row_custody_packets_v2` have no caller outside their own tests;
//! the artifact path has never executed a `Collect` on chain; and each side's
//! tests assert against its own author. GEN-HOT's lesson generalises one level
//! further than it was written: **a family's own emitter and its own
//! authenticator are not two authorities, and neither are a family's own
//! contract and its own artifact.**
//!
//! The repair is not to correct the second copy. It is to delete it.
//! [`general_child_custody_movement_v1`] is the one place General says which
//! compartments a child effect moves between and which identity keys each side's
//! vault; the artifact builder, the artifact join and the packet builder all
//! read it, so a future ruling moves one table and the three cannot drift again.

use dclutch_custody_contract::CompartmentV1;
use dclutch_general_codec::Action;

use crate::GeneralChildEffectV1;
use crate::candidate_v1::{
    GeneralCandidateOpeningV1, GeneralCandidateV1, WorkCompartmentV1, WorkRewardV1,
};
use crate::collection_v1::{
    EscrowDirectionV1, GeneralBatchV1, GeneralOrderPhaseV1, GeneralOrderV1, OrderEscrowV1,
};

// ---------------------------------------------------------------------------
// §3 -- the compartment and vault-context authority
// ---------------------------------------------------------------------------

/// Which identity one side of a Custody transfer is keyed by.
///
/// A Custody vault PDA folds `(market, release_set, context, compartment)`, and
/// the CONTEXT is what separates two pools carrying the same compartment tag.
/// Decision 0010 §2 declined a new tag for exactly this reason: an order's
/// escrow and a candidate's settlement inventory are the same economic pool and
/// must stay interconvertible, so what distinguishes them is a seed and not a
/// taxonomy row. Naming the seed here is what makes that argument checkable
/// rather than a comment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultContextV1 {
    /// No vault: an externally owned token account, keyed by its owner.
    External,
    /// One order's own content identity: that order's escrow, and nothing else.
    Order,
    /// One candidate's content identity: the settlement inventory.
    Candidate,
    /// The Market identity: the Hoard.
    Market,
}

impl VaultContextV1 {
    /// Whether this side names a Custody vault rather than an external account.
    #[must_use]
    pub const fn is_vault(self) -> bool {
        !matches!(self, Self::External)
    }
}

/// The exact Custody movement one General child effect performs.
///
/// Both halves are here on purpose. Custody's `Transfer` validation ties the
/// compartment tag and the vault context together -- an `External` side must
/// carry a zero context and a nonzero owner, a vault side the reverse -- so a
/// table that named only the compartments would leave the other half of the
/// same fact to be restated somewhere else, which is the defect this module
/// exists to retire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustodyMovementV1 {
    /// Compartment the atoms leave.
    pub source_compartment: CompartmentV1,
    /// Compartment the atoms arrive in.
    pub destination_compartment: CompartmentV1,
    /// Identity the source vault is keyed by.
    pub source_context: VaultContextV1,
    /// Identity the destination vault is keyed by.
    pub destination_context: VaultContextV1,
}

impl CustodyMovementV1 {
    const fn new(
        source_compartment: CompartmentV1,
        source_context: VaultContextV1,
        destination_compartment: CompartmentV1,
        destination_context: VaultContextV1,
    ) -> Self {
        Self {
            source_compartment,
            destination_compartment,
            source_context,
            destination_context,
        }
    }

    /// Whether this movement's shape satisfies Custody's own `Transfer` rule.
    ///
    /// Custody requires `(compartment == External) == (context is external)` on
    /// both sides. Restating that rule here is not duplication: it is what makes
    /// a wrong row in the table below a compile-time-adjacent refusal instead of
    /// a runtime `InvalidOperationShape` from a child program.
    #[must_use]
    pub const fn is_custody_admissible(self) -> bool {
        matches!(self.source_compartment, CompartmentV1::External)
            == matches!(self.source_context, VaultContextV1::External)
            && matches!(self.destination_compartment, CompartmentV1::External)
                == matches!(self.destination_context, VaultContextV1::External)
    }
}

/// Return the sole Custody movement one child effect performs, if it has one.
///
/// `None` is not "unknown": it means the effect moves Claims rather than
/// collateral, and it is what the Claims-only effects return.
#[must_use]
pub const fn general_child_custody_movement_v1(
    effect: GeneralChildEffectV1,
) -> Option<CustodyMovementV1> {
    use CompartmentV1::{External, HoardPrincipal, Settlement};
    use VaultContextV1 as Ctx;
    match effect {
        // Claims legs. They move Positions, not vault balances.
        GeneralChildEffectV1::CollectClaims
        | GeneralChildEffectV1::DistributeClaims
        | GeneralChildEffectV1::EscrowClaims
        | GeneralChildEffectV1::ReleaseClaims => None,
        // THE ESCROW RULING, in the one place it is now written. Decision 0010
        // §2: a `Collect` draws on collateral the protocol is already holding in
        // the ORDER's own vault. The `External` source this replaced could reach
        // an account the maker still controlled.
        GeneralChildEffectV1::CollectCollateral => Some(CustodyMovementV1::new(
            Settlement,
            Ctx::Order,
            Settlement,
            Ctx::Candidate,
        )),
        GeneralChildEffectV1::DistributeCollateral | GeneralChildEffectV1::PaySurplus => Some(
            CustodyMovementV1::new(Settlement, Ctx::Candidate, External, Ctx::External),
        ),
        GeneralChildEffectV1::MintCompleteSet => Some(CustodyMovementV1::new(
            Settlement,
            Ctx::Candidate,
            HoardPrincipal,
            Ctx::Market,
        )),
        GeneralChildEffectV1::MergeCompleteSet => Some(CustodyMovementV1::new(
            HoardPrincipal,
            Ctx::Market,
            Settlement,
            Ctx::Candidate,
        )),
        // Admission moves the maker's worst case IN; cancellation and post-window
        // release move the remainder back OUT. Both address the escrow by the
        // order's own content identity, which is what makes a refund exact
        // without a ledger of what each order consumed.
        GeneralChildEffectV1::EscrowCollateral => Some(CustodyMovementV1::new(
            External,
            Ctx::External,
            Settlement,
            Ctx::Order,
        )),
        GeneralChildEffectV1::ReleaseCollateral => Some(CustodyMovementV1::new(
            Settlement,
            Ctx::Order,
            External,
            Ctx::External,
        )),
    }
}

/// The four identities a General child effect may key a transfer side by.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MovementIdentitiesV1 {
    /// Content identity of the order whose escrow is in play, if any.
    pub order_id: [u8; 32],
    /// Content identity of the candidate whose inventory is in play, if any.
    pub candidate_id: [u8; 32],
    /// Canonical Market identity, which keys the Hoard.
    pub market_id: [u8; 32],
    /// Maker or beneficiary on the external side.
    pub owner_id: [u8; 32],
}

/// Resolve one side of a movement into its exact `(vault_context, owner)` pair.
///
/// Custody requires exactly one of the two to be nonzero, and which one is a
/// consequence of the compartment. Returning both from one place is what stops
/// a caller pairing a vault compartment with an owner, or the reverse, and
/// discovering it as an `InvalidOperationShape` from a child program.
#[must_use]
pub const fn resolve_vault_side_v1(
    context: VaultContextV1,
    identities: MovementIdentitiesV1,
) -> ([u8; 32], [u8; 32]) {
    match context {
        VaultContextV1::External => ([0; 32], identities.owner_id),
        VaultContextV1::Order => (identities.order_id, [0; 32]),
        VaultContextV1::Candidate => (identities.candidate_id, [0; 32]),
        VaultContextV1::Market => (identities.market_id, [0; 32]),
    }
}

/// Require one observed Custody frame to be exactly the movement `effect` names.
///
/// The packet builder calls this instead of restating the route's identities.
/// Before it, `build_row_custody_packets_v2` carried its own literal reading of
/// which side was a vault and which was an owner -- a third copy of the fact
/// this module now owns, and the copy that happened to be right.
pub fn authenticate_custody_route_v1(
    effect: GeneralChildEffectV1,
    identities: MovementIdentitiesV1,
    source_vault_context: [u8; 32],
    source_owner: [u8; 32],
    destination_vault_context: [u8; 32],
    destination_owner: [u8; 32],
) -> GeneralEscrowResultV1<CustodyMovementV1> {
    let movement =
        general_child_custody_movement_v1(effect).ok_or(GeneralEscrowErrorV1::Substitution)?;
    let (expected_source_context, expected_source_owner) =
        resolve_vault_side_v1(movement.source_context, identities);
    let (expected_destination_context, expected_destination_owner) =
        resolve_vault_side_v1(movement.destination_context, identities);
    if source_vault_context != expected_source_context
        || source_owner != expected_source_owner
        || destination_vault_context != expected_destination_context
        || destination_owner != expected_destination_owner
    {
        return Err(GeneralEscrowErrorV1::Substitution);
    }
    Ok(movement)
}

/// The Custody transfer route one action's authored artifact declares.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActionCustodyTransferV1 {
    /// The action declares no Custody `Transfer` route.
    None,
    /// One direction, fixed in the template's two compartment bytes.
    Fixed(GeneralChildEffectV1),
    /// One template and two admissible directions, selected by the
    /// authenticated complete-set move and patched into the compartment bytes.
    Either(GeneralChildEffectV1, GeneralChildEffectV1),
}

/// Return the Custody transfer one action performs.
///
/// This is the join between the action-selected artifact and the effect-selected
/// movement. `effect_artifacts_v3` builds its template from it and
/// `artifacts_v3` admits a release against it, so the artifact and its
/// authenticator read one table rather than two literals.
#[must_use]
pub const fn general_action_custody_transfer_v1(action: Action) -> ActionCustodyTransferV1 {
    match action {
        Action::Collect => ActionCustodyTransferV1::Fixed(GeneralChildEffectV1::CollectCollateral),
        Action::Distribute => {
            ActionCustodyTransferV1::Fixed(GeneralChildEffectV1::DistributeCollateral)
        }
        Action::Materialize => ActionCustodyTransferV1::Either(
            GeneralChildEffectV1::MintCompleteSet,
            GeneralChildEffectV1::MergeCompleteSet,
        ),
        // Close's route 0 pays the exact terminal quote surplus out of the
        // candidate's settlement vault; its other three routes are lifecycle,
        // not transfers.
        Action::Close => ActionCustodyTransferV1::Fixed(GeneralChildEffectV1::PaySurplus),
        Action::PlaceOrder => {
            ActionCustodyTransferV1::Fixed(GeneralChildEffectV1::EscrowCollateral)
        }
        Action::CancelOrder | Action::ReleaseOrder => {
            ActionCustodyTransferV1::Fixed(GeneralChildEffectV1::ReleaseCollateral)
        }
        Action::Consider
        | Action::Freeze
        | Action::InitializeSettlement
        | Action::OpenBatch
        | Action::CloseBatch
        | Action::SubmitCandidate
        | Action::VerifyCandidateRow
        | Action::CloseCandidate => ActionCustodyTransferV1::None,
    }
}

/// Return the compartments one action's Custody template must carry.
///
/// For an [`ActionCustodyTransferV1::Either`] action the template carries the
/// FIRST direction and the EffectProgram patches the two bytes; the join admits
/// either.
#[must_use]
pub const fn general_action_template_compartments_v1(
    action: Action,
) -> Option<(CompartmentV1, CompartmentV1)> {
    let effect = match general_action_custody_transfer_v1(action) {
        ActionCustodyTransferV1::None => return None,
        ActionCustodyTransferV1::Fixed(effect) | ActionCustodyTransferV1::Either(effect, _) => {
            effect
        }
    };
    match general_child_custody_movement_v1(effect) {
        None => None,
        Some(movement) => Some((
            movement.source_compartment,
            movement.destination_compartment,
        )),
    }
}

// ---------------------------------------------------------------------------
// §1 -- the work escrow, physically
// ---------------------------------------------------------------------------

/// Stable refusal from one physical escrow observation or movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralEscrowErrorV1 {
    /// The observed balance is not the balance the record's accounting claims.
    Uncapitalized,
    /// A funding leg did not carry exactly the required lamports or atoms.
    Unfunded,
    /// A draw would reach past its compartment, or into the rent floor.
    Overdrawn,
    /// The observed post-balance differed from the exact planned one.
    PostconditionMismatch,
    /// An escrow was addressed by an identity that is not its order's.
    Substitution,
    /// The order's phase does not admit this movement.
    InvalidOrderPhase,
    /// A required floor, identity, or width was zero.
    ZeroCoordinate,
    /// Checked lamport or atom arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for physical escrow observation and movement.
pub type GeneralEscrowResultV1<T> = core::result::Result<T, GeneralEscrowErrorV1>;

/// Observed lamport balances one work-escrow transition reads.
///
/// The work escrow is the submission record's own account. Nothing separates
/// the two compartments physically -- they share one balance -- so the record's
/// split is meaningful only while the TOTAL is re-proven against it. That is
/// what [`authenticate_work_escrow_v1`] does, and it is why every plan below is
/// constructed from an observation rather than from the record alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkEscrowObservationV1 {
    /// Lamports the submission record's account holds right now.
    pub escrow_lamports: u64,
    /// Rent-exempt minimum for that account at its exact width.
    ///
    /// It is a floor, never a compartment: a crank paid out of it would leave
    /// the record collectable, and the refusal would arrive later as an
    /// unrelated rent failure on some other instruction.
    pub rent_floor: u64,
    /// Lamports the actor this transition pays holds right now.
    pub beneficiary_lamports: u64,
}

/// Exact lamports one submission's account must hold at a given state.
pub fn work_escrow_required_lamports_v1(
    submission: GeneralCandidateV1,
    rent_floor: u64,
) -> GeneralEscrowResultV1<u64> {
    let state = submission.state();
    rent_floor
        .checked_add(state.verification_remaining)
        .and_then(|value| value.checked_add(state.cleanup_remaining))
        .ok_or(GeneralEscrowErrorV1::ArithmeticOverflow)
}

/// Re-prove that the account holds exactly what the record says it holds.
///
/// Two conjuncts, and they are independent. The first is the record's own
/// consistency, which is what
/// [`GeneralCandidateV1::validate_capitalization`](crate::candidate_v1::GeneralCandidateV1::validate_capitalization)
/// already proved. The second is the one that was missing: the account's
/// observed lamports equal `rent_floor + verification_remaining +
/// cleanup_remaining`. Without it a submission could claim any capitalization it
/// liked and every transition would agree with it, because every transition was
/// reading the claim.
pub fn authenticate_work_escrow_v1(
    submission: GeneralCandidateV1,
    rows_verified: u32,
    observation: WorkEscrowObservationV1,
) -> GeneralEscrowResultV1<()> {
    if observation.rent_floor == 0 {
        return Err(GeneralEscrowErrorV1::ZeroCoordinate);
    }
    submission
        .validate_capitalization(rows_verified)
        .map_err(|_| GeneralEscrowErrorV1::Uncapitalized)?;
    if observation.escrow_lamports
        != work_escrow_required_lamports_v1(submission, observation.rent_floor)?
    {
        return Err(GeneralEscrowErrorV1::Uncapitalized);
    }
    Ok(())
}

/// One exact lamport movement funding a submission's work escrow.
///
/// The solver pays the record's rent AND its whole work capacity in one
/// movement, because a record that exists without its capacity is a candidate
/// nobody can be paid to verify -- and the funded-walk discipline is that a
/// permissionless verb is live only when its reward is already held.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkEscrowFundingPlanV1 {
    solver_before: u64,
    solver_after: u64,
    escrow_before: u64,
    escrow_after: u64,
    rent_floor: u64,
    work_capacity: u64,
}

impl WorkEscrowFundingPlanV1 {
    /// Build the sole admitted submission-funding movement.
    ///
    /// Exact in both directions, exactly as
    /// [`GeneralCandidateV1::submit`](crate::candidate_v1::GeneralCandidateV1::submit)
    /// is: underfunding buys work nobody is paid for, and overfunding leaves
    /// lamports in a compartment with no rule for who gets them. SRC-FOUND
    /// stated the same thing from the other side -- over-funding is not a
    /// donation a prepaid compartment may keep.
    pub fn new(
        opening: GeneralCandidateOpeningV1,
        rent_floor: u64,
        solver_before: u64,
        escrow_before: u64,
    ) -> GeneralEscrowResultV1<Self> {
        if rent_floor == 0 {
            return Err(GeneralEscrowErrorV1::ZeroCoordinate);
        }
        // A submission record is created by this movement, so its account must
        // be vacant. A nonzero balance here is either a replay or a stranger's
        // prepayment, and neither has a rule.
        if escrow_before != 0 {
            return Err(GeneralEscrowErrorV1::Unfunded);
        }
        let work_capacity = opening
            .work_capacity()
            .map_err(|_| GeneralEscrowErrorV1::ArithmeticOverflow)?;
        let escrow_after = rent_floor
            .checked_add(work_capacity)
            .ok_or(GeneralEscrowErrorV1::ArithmeticOverflow)?;
        let solver_after = solver_before
            .checked_sub(escrow_after)
            .ok_or(GeneralEscrowErrorV1::Unfunded)?;
        Ok(Self {
            solver_before,
            solver_after,
            escrow_before,
            escrow_after,
            rent_floor,
            work_capacity,
        })
    }

    /// Verify observed post-balances against this exact plan.
    pub fn validate_post(self, solver_after: u64, escrow_after: u64) -> GeneralEscrowResultV1<()> {
        if solver_after != self.solver_after || escrow_after != self.escrow_after {
            return Err(GeneralEscrowErrorV1::PostconditionMismatch);
        }
        Ok(())
    }

    /// Solver lamports before funding.
    #[must_use]
    pub const fn solver_before(self) -> u64 {
        self.solver_before
    }
    /// Solver lamports after funding.
    #[must_use]
    pub const fn solver_after(self) -> u64 {
        self.solver_after
    }
    /// Escrow lamports before funding; always zero.
    #[must_use]
    pub const fn escrow_before(self) -> u64 {
        self.escrow_before
    }
    /// Escrow lamports after funding.
    #[must_use]
    pub const fn escrow_after(self) -> u64 {
        self.escrow_after
    }
    /// Exact rent floor this plan funded.
    #[must_use]
    pub const fn rent_floor(self) -> u64 {
        self.rent_floor
    }
    /// Exact work capacity this plan funded.
    #[must_use]
    pub const fn work_capacity(self) -> u64 {
        self.work_capacity
    }
}

/// One exact crank payment out of a candidate's own work escrow.
///
/// This is the funded-permissionless-walk discipline, applied where decision
/// 0010 §1 said it applies: the reward is already held, so performing the verb
/// is paid rather than hoped for. The plan and the successor record are
/// constructed together, so a transition cannot debit the compartment in its
/// record and leave the account untouched, or the reverse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkEscrowDrawPlanV1 {
    escrow_before: u64,
    escrow_after: u64,
    beneficiary_before: u64,
    beneficiary_after: u64,
    reward: WorkRewardV1,
}

impl WorkEscrowDrawPlanV1 {
    /// Build the sole admitted crank payment for one completed transition.
    ///
    /// `successor` is the submission record AFTER the transition and
    /// `rows_verified_after` its row cursor there. The construction proves three
    /// things at once: the predecessor account held exactly what the predecessor
    /// record claimed, the successor account holds exactly what the successor
    /// record claims, and the difference is exactly one reward. Any two of those
    /// can be made to agree by a wrong third; requiring all three is what makes
    /// the compartment split mean something in an account that does not have
    /// two balances.
    pub fn new(
        before: WorkEscrowObservationV1,
        successor: GeneralCandidateV1,
        rows_verified_after: u32,
        reward: WorkRewardV1,
    ) -> GeneralEscrowResultV1<Self> {
        if before.rent_floor == 0 {
            return Err(GeneralEscrowErrorV1::ZeroCoordinate);
        }
        if reward.lamports == 0 {
            return Err(GeneralEscrowErrorV1::ZeroCoordinate);
        }
        successor
            .validate_capitalization(rows_verified_after)
            .map_err(|_| GeneralEscrowErrorV1::Uncapitalized)?;
        let escrow_after = before
            .escrow_lamports
            .checked_sub(reward.lamports)
            .ok_or(GeneralEscrowErrorV1::Overdrawn)?;
        // A crank may never be paid out of the rent floor. Without this the
        // route could pay out of an account it was about to leave collectable,
        // and the refusal would arrive later, somewhere else, as a rent failure
        // nobody could attribute to a crank.
        if escrow_after < before.rent_floor {
            return Err(GeneralEscrowErrorV1::Overdrawn);
        }
        if escrow_after != work_escrow_required_lamports_v1(successor, before.rent_floor)? {
            return Err(GeneralEscrowErrorV1::Uncapitalized);
        }
        let beneficiary_after = before
            .beneficiary_lamports
            .checked_add(reward.lamports)
            .ok_or(GeneralEscrowErrorV1::ArithmeticOverflow)?;
        Ok(Self {
            escrow_before: before.escrow_lamports,
            escrow_after,
            beneficiary_before: before.beneficiary_lamports,
            beneficiary_after,
            reward,
        })
    }

    /// Verify observed post-balances against this exact plan.
    pub fn validate_post(
        self,
        escrow_after: u64,
        beneficiary_after: u64,
    ) -> GeneralEscrowResultV1<()> {
        if escrow_after != self.escrow_after || beneficiary_after != self.beneficiary_after {
            return Err(GeneralEscrowErrorV1::PostconditionMismatch);
        }
        Ok(())
    }

    /// Escrow lamports before the draw.
    #[must_use]
    pub const fn escrow_before(self) -> u64 {
        self.escrow_before
    }
    /// Escrow lamports after the draw.
    #[must_use]
    pub const fn escrow_after(self) -> u64 {
        self.escrow_after
    }
    /// Beneficiary lamports before the draw.
    #[must_use]
    pub const fn beneficiary_before(self) -> u64 {
        self.beneficiary_before
    }
    /// Beneficiary lamports after the draw.
    #[must_use]
    pub const fn beneficiary_after(self) -> u64 {
        self.beneficiary_after
    }
    /// The exact reward this draw paid, and the compartment it came from.
    #[must_use]
    pub const fn reward(self) -> WorkRewardV1 {
        self.reward
    }
}

/// The exact three-way movement that closes one spent candidate out.
///
/// Decision 0010 §6 item 3 names rent ownership as designed and not moved: "the
/// solver owns a submission's rent and the maker owns an order's, and closure
/// has no route." This is that route for the submission half. The cleanup crank
/// goes to whoever performed it; the unspent verification compartment AND the
/// rent go back to the solver, because a candidate that lost -- or that nobody
/// finished verifying -- must not pay a stranger for work not done, and must not
/// forfeit an account deposit for losing a comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkEscrowClosePlanV1 {
    escrow_before: u64,
    cranker_before: u64,
    cranker_after: u64,
    solver_before: u64,
    solver_after: u64,
    cleanup_reward: u64,
    solver_credit: u64,
}

impl WorkEscrowClosePlanV1 {
    /// Build the sole admitted close-out movement.
    ///
    /// `cleanup` and `solver_refund` are exactly what
    /// [`GeneralCandidateV1::close_out`](crate::candidate_v1::GeneralCandidateV1::close_out)
    /// returned. The conservation conjunct is the point: everything the account
    /// held is accounted for by the two credits, so a close cannot strand
    /// lamports in an account it is about to leave at zero length, and cannot
    /// pay out more than it held.
    pub fn new(
        before: WorkEscrowObservationV1,
        cleanup: WorkRewardV1,
        solver_refund: u64,
        solver_before: u64,
    ) -> GeneralEscrowResultV1<Self> {
        if before.rent_floor == 0 || cleanup.lamports == 0 {
            return Err(GeneralEscrowErrorV1::ZeroCoordinate);
        }
        if cleanup.compartment != WorkCompartmentV1::Cleanup {
            return Err(GeneralEscrowErrorV1::Substitution);
        }
        // The rent goes back with the residual: one credit, one owner.
        let solver_credit = solver_refund
            .checked_add(before.rent_floor)
            .ok_or(GeneralEscrowErrorV1::ArithmeticOverflow)?;
        let disbursed = cleanup
            .lamports
            .checked_add(solver_credit)
            .ok_or(GeneralEscrowErrorV1::ArithmeticOverflow)?;
        if disbursed != before.escrow_lamports {
            return Err(GeneralEscrowErrorV1::Uncapitalized);
        }
        let cranker_after = before
            .beneficiary_lamports
            .checked_add(cleanup.lamports)
            .ok_or(GeneralEscrowErrorV1::ArithmeticOverflow)?;
        let solver_after = solver_before
            .checked_add(solver_credit)
            .ok_or(GeneralEscrowErrorV1::ArithmeticOverflow)?;
        Ok(Self {
            escrow_before: before.escrow_lamports,
            cranker_before: before.beneficiary_lamports,
            cranker_after,
            solver_before,
            solver_after,
            cleanup_reward: cleanup.lamports,
            solver_credit,
        })
    }

    /// Verify observed post-balances against this exact plan.
    ///
    /// The escrow's post-balance is not a parameter of the plan and is required
    /// to be zero: a closed submission account keeps nothing, and a residual
    /// there would be a fourth party to a three-way movement.
    pub fn validate_post(
        self,
        escrow_after: u64,
        cranker_after: u64,
        solver_after: u64,
    ) -> GeneralEscrowResultV1<()> {
        if escrow_after != 0
            || cranker_after != self.cranker_after
            || solver_after != self.solver_after
        {
            return Err(GeneralEscrowErrorV1::PostconditionMismatch);
        }
        Ok(())
    }

    /// Escrow lamports before the close.
    #[must_use]
    pub const fn escrow_before(self) -> u64 {
        self.escrow_before
    }
    /// Cranker lamports before the close.
    #[must_use]
    pub const fn cranker_before(self) -> u64 {
        self.cranker_before
    }
    /// Cranker lamports after the close.
    #[must_use]
    pub const fn cranker_after(self) -> u64 {
        self.cranker_after
    }
    /// Solver lamports before the close.
    #[must_use]
    pub const fn solver_before(self) -> u64 {
        self.solver_before
    }
    /// Solver lamports after the close.
    #[must_use]
    pub const fn solver_after(self) -> u64 {
        self.solver_after
    }
    /// Exact cleanup reward paid to the closing actor.
    #[must_use]
    pub const fn cleanup_reward(self) -> u64 {
        self.cleanup_reward
    }
    /// Exact residual plus rent returned to the solver.
    #[must_use]
    pub const fn solver_credit(self) -> u64 {
        self.solver_credit
    }
}

// ---------------------------------------------------------------------------
// §2 -- the order escrow, physically
// ---------------------------------------------------------------------------

/// Observed collateral balances one order-escrow movement reads.
///
/// The claim leg is deliberately absent from this struct and checked by
/// [`authenticate_order_escrow_claims_v1`] one outcome at a time. A runtime
/// width reaches 258 outcomes and two `[u64; 258]` arrays are four kilobytes on
/// a four-kilobyte SBF frame; the tree has already paid for that lesson once, in
/// `plan_and_encode_deadline_failure`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderEscrowObservationV1 {
    /// Identity the observed escrow vault is keyed by.
    ///
    /// This is the cross-order and cross-batch guard, and it is the whole reason
    /// the escrow is per-order: an escrow reachable by an identity other than
    /// its own order's is one maker's collateral in another maker's refund.
    pub escrow_context: [u8; 32],
    /// Quote atoms the order's escrow vault holds right now.
    pub vault_quote_atoms: u64,
    /// Quote atoms the maker's external account holds right now.
    pub maker_quote_atoms: u64,
}

/// One exact quote movement in or out of one order's escrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderEscrowPlanV1 {
    order_id: [u8; 32],
    owner_id: [u8; 32],
    direction: EscrowDirectionV1,
    vault_before: u64,
    vault_after: u64,
    maker_before: u64,
    maker_after: u64,
    quote_atoms: u64,
}

impl OrderEscrowPlanV1 {
    /// Build the sole admitted physical movement for one authenticated escrow.
    ///
    /// `escrow` is exactly what `admit`, `cancel` or `release` returned. Nothing
    /// here recomputes an amount those transitions already fixed; what it adds
    /// is the balance each direction requires, which is the difference between
    /// an accounted escrow and a held one.
    pub fn new(
        batch: GeneralBatchV1,
        order: GeneralOrderV1<'_>,
        escrow: OrderEscrowV1,
        observation: OrderEscrowObservationV1,
    ) -> GeneralEscrowResultV1<Self> {
        let header = order.header();
        if header.batch_id != batch.batch_id() {
            return Err(GeneralEscrowErrorV1::Substitution);
        }
        let order_id = order.order_id();
        if escrow.order_id != order_id
            || escrow.owner_id != header.owner_id
            || observation.escrow_context != order_id
        {
            return Err(GeneralEscrowErrorV1::Substitution);
        }
        if order.state().phase != GeneralOrderPhaseV1::Placed {
            return Err(GeneralEscrowErrorV1::InvalidOrderPhase);
        }
        let reserve = order
            .quote_reserve()
            .map_err(|_| GeneralEscrowErrorV1::ArithmeticOverflow)?;
        let (vault_after, maker_after, quote_atoms) = match escrow.direction {
            EscrowDirectionV1::Deposit => {
                // A fresh escrow. A nonzero balance here is a replayed admission
                // or a stranger's deposit, and admitting either would let a
                // later refund pay out atoms this order never reserved.
                if observation.vault_quote_atoms != 0 {
                    return Err(GeneralEscrowErrorV1::Unfunded);
                }
                if escrow.quote_atoms != reserve {
                    return Err(GeneralEscrowErrorV1::Unfunded);
                }
                let maker_after = observation
                    .maker_quote_atoms
                    .checked_sub(reserve)
                    .ok_or(GeneralEscrowErrorV1::Unfunded)?;
                (reserve, maker_after, reserve)
            }
            EscrowDirectionV1::Refund => {
                // Cancellation is legal only while the batch collects, so no
                // `Collect` can have drawn on this vault yet and the whole
                // reserve must still be there. A vault holding anything else is
                // not this order's escrow in the state this transition assumed.
                if observation.vault_quote_atoms != reserve || escrow.quote_atoms != reserve {
                    return Err(GeneralEscrowErrorV1::Uncapitalized);
                }
                let maker_after = observation
                    .maker_quote_atoms
                    .checked_add(reserve)
                    .ok_or(GeneralEscrowErrorV1::ArithmeticOverflow)?;
                (0, maker_after, reserve)
            }
            EscrowDirectionV1::Residual => {
                // The residual is not computed and not quoted: whatever a
                // winning candidate collected has already left, so what remains
                // IS the refund. What this adds is the bound the address was
                // said to give for free -- a maker can never be paid more than
                // they escrowed -- now checked rather than asserted.
                if escrow.quote_atoms != 0 {
                    return Err(GeneralEscrowErrorV1::Substitution);
                }
                if observation.vault_quote_atoms > reserve {
                    return Err(GeneralEscrowErrorV1::Uncapitalized);
                }
                let maker_after = observation
                    .maker_quote_atoms
                    .checked_add(observation.vault_quote_atoms)
                    .ok_or(GeneralEscrowErrorV1::ArithmeticOverflow)?;
                (0, maker_after, observation.vault_quote_atoms)
            }
        };
        Ok(Self {
            order_id,
            owner_id: header.owner_id,
            direction: escrow.direction,
            vault_before: observation.vault_quote_atoms,
            vault_after,
            maker_before: observation.maker_quote_atoms,
            maker_after,
            quote_atoms,
        })
    }

    /// Verify observed post-balances against this exact plan.
    pub fn validate_post(self, vault_after: u64, maker_after: u64) -> GeneralEscrowResultV1<()> {
        if vault_after != self.vault_after || maker_after != self.maker_after {
            return Err(GeneralEscrowErrorV1::PostconditionMismatch);
        }
        Ok(())
    }

    /// Content identity of the order whose escrow this moves.
    #[must_use]
    pub const fn order_id(self) -> [u8; 32] {
        self.order_id
    }
    /// Maker identity on the external side.
    #[must_use]
    pub const fn owner_id(self) -> [u8; 32] {
        self.owner_id
    }
    /// Which way the movement runs.
    #[must_use]
    pub const fn direction(self) -> EscrowDirectionV1 {
        self.direction
    }
    /// Vault atoms before the movement.
    #[must_use]
    pub const fn vault_before(self) -> u64 {
        self.vault_before
    }
    /// Vault atoms after the movement.
    #[must_use]
    pub const fn vault_after(self) -> u64 {
        self.vault_after
    }
    /// Maker atoms before the movement.
    #[must_use]
    pub const fn maker_before(self) -> u64 {
        self.maker_before
    }
    /// Maker atoms after the movement.
    #[must_use]
    pub const fn maker_after(self) -> u64 {
        self.maker_after
    }
    /// Exact atoms this movement carries.
    #[must_use]
    pub const fn quote_atoms(self) -> u64 {
        self.quote_atoms
    }
}

/// Require one order's escrowed claims to match its reserve at one outcome.
///
/// Called once per runtime outcome so no fixed-capacity array enters an SBF
/// frame. `escrowed` is the escrow Position's observed magnitude at `outcome`.
pub fn authenticate_order_escrow_claims_v1(
    order: GeneralOrderV1<'_>,
    direction: EscrowDirectionV1,
    outcome: u32,
    escrowed: u64,
) -> GeneralEscrowResultV1<()> {
    let reserve = order
        .claim_reserve(outcome)
        .map_err(|_| GeneralEscrowErrorV1::ArithmeticOverflow)?;
    let admissible = match direction {
        // Before admission the escrow Position holds nothing at this outcome.
        EscrowDirectionV1::Deposit => escrowed == 0,
        // Cancellation returns the whole reserve, untouched.
        EscrowDirectionV1::Refund => escrowed == reserve,
        // A post-window release returns whatever settlement left, and that can
        // never exceed what admission put there.
        EscrowDirectionV1::Residual => escrowed <= reserve,
    };
    if admissible {
        Ok(())
    } else {
        Err(GeneralEscrowErrorV1::Uncapitalized)
    }
}

/// Require one settlement row to draw only on the escrow its order holds.
///
/// This is the physical half of decision 0010 §2's central claim. The pure
/// transition already refuses a row whose order is not `Placed`; what it cannot
/// see is whether the vault that row names actually holds the debit. Without
/// this a candidate could be verified against an escrow, and settled against a
/// vault that had been emptied or that belongs to another order entirely.
pub fn authenticate_collect_from_escrow_v1(
    batch: GeneralBatchV1,
    order: GeneralOrderV1<'_>,
    observation: OrderEscrowObservationV1,
    quote_debit: u64,
) -> GeneralEscrowResultV1<()> {
    if order.header().batch_id != batch.batch_id() {
        return Err(GeneralEscrowErrorV1::Substitution);
    }
    if observation.escrow_context != order.order_id() {
        return Err(GeneralEscrowErrorV1::Substitution);
    }
    if order.state().phase != GeneralOrderPhaseV1::Placed {
        return Err(GeneralEscrowErrorV1::InvalidOrderPhase);
    }
    if observation.vault_quote_atoms < quote_debit {
        return Err(GeneralEscrowErrorV1::Overdrawn);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
