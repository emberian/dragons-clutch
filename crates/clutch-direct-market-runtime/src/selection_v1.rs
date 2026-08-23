// SPDX-License-Identifier: AGPL-3.0-or-later

//! Fresh `0xb2/1` Direct Selection semantic owner.
//!
//! The complete live Reservation prefix is projected once into the exact
//! owner-blind two-row RelationV2 book. Candidate submission, traversal, and
//! ranking never accept an alternate caller-shaped book. Every retained
//! candidate is checked at admission and rechecked at its canonical cursor;
//! finalization selects only the best valid submitted candidate.

use core::cmp::Ordering;

use clutch_batch::direct_pair_v1::{
    authenticate_compact_selected_direct_pair_v1, verify_compact_direct_candidate_v1,
    AuthenticatedDirectSelectionAuthorityV1, DirectEconomicBookV1,
    DirectEconomicCandidateV1, DirectPairErrorV1, SelectedDirectPairV1,
};
use clutch_batch::relation_v2::{
    EconomicDomainV2, EconomicOrderV2, PricePreconditionV2, VerifiedEconomicsV2,
    ECONOMIC_RELATION_VERSION_V2, EMPTY_ECONOMIC_ORDER_V2,
};
use clutch_batch::Side;

use crate::reservation_v1::{DirectReservationPhaseV1, DirectReservationV1};
use crate::{
    require_fresh_child_account, require_live, DirectHashBackendV1, DirectMarketErrorV1,
    DirectMarketRootV1, DirectRentOwnerV1, DirectRootPhaseV1, DirectRootReplayPostV1,
    MAX_DIRECT_CANDIDATES_V1, MAX_DIRECT_RESERVATIONS_V1,
};

const SELECTION_STATE_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/selection-state/v1\0";
const SELECTION_BOOK_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/selection-book/v1\0";
const SELECTION_ORDER_DOMAIN_V1: &[u8] = b"dragons-clutch/direct/selection-order/v1\0";
const SELECTION_TRAVERSAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/selection-traversal/v1\0";
const SELECTED_TRAVERSAL_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/selected-traversal/v1\0";

const _: () = assert!(MAX_DIRECT_RESERVATIONS_V1 == 2);
const _: () = assert!(MAX_DIRECT_CANDIDATES_V1 == 3);
const CANDIDATE_CAPACITY_V1: usize = 3;

/// Exhaustive persisted Selection phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectSelectionPhaseV1 {
    /// Fewer than two live Reservations froze; no candidate is possible.
    FrozenEmpty,
    /// The exact two-row book accepts bounded candidate submissions.
    SubmissionOpen,
    /// The retained prefix is being reverified in submission order.
    Verifying,
    /// The best valid submitted candidate has been selected exactly once.
    Selected,
    /// Action 9..12 fixed the immutable economic terminal receipt.
    Terminal,
}

impl DirectSelectionPhaseV1 {
    /// Stable persisted byte.
    pub const fn byte(self) -> u8 {
        match self {
            Self::FrozenEmpty => 1,
            Self::SubmissionOpen => 2,
            Self::Verifying => 3,
            Self::Selected => 4,
            Self::Terminal => 5,
        }
    }
}

/// Default-deny adapter boundary for action 4's complete account projection.
pub trait AuthenticatedDirectSelectionFreezeV1 {
    /// Authenticate the exact writable root/replay, fresh Selection PDA, every
    /// live `0xb4/1` account and semantic ID, immutable price authority, rent,
    /// and the absence of any missing or extra Reservation.
    fn authenticate_freeze(
        &self,
        _root: DirectMarketRootV1,
        _selection_account: [u8; 32],
        _rent: DirectRentOwnerV1,
        _reservations: &[Option<DirectReservationV1>; 2],
        _reservation_semantic_ids: &[[u8; 32]; 2],
        _domain: &EconomicDomainV2,
        _price: &PricePreconditionV2,
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing freeze authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectSelectionFreezeAuthorityV1;

impl AuthenticatedDirectSelectionFreezeV1 for NoDirectSelectionFreezeAuthorityV1 {}

/// Sole persisted owner of the frozen Direct book and selected traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSelectionV1 {
    market_instance_id: [u8; 32],
    generation: u64,
    direct_root_account: [u8; 32],
    selection_account: [u8; 32],
    reservation_accounts: [[u8; 32]; 2],
    reservation_semantic_ids: [[u8; 32]; 2],
    reservation_count: u8,
    domain: EconomicDomainV2,
    book: DirectEconomicBookV1,
    price: PricePreconditionV2,
    candidates: [DirectEconomicCandidateV1; CANDIDATE_CAPACITY_V1],
    candidate_digests: [[u8; 32]; CANDIDATE_CAPACITY_V1],
    candidate_count: u8,
    verification_cursor: u8,
    verified_mask: u8,
    traversal_transcript_id: [u8; 32],
    selected_candidate_index: Option<u8>,
    selected_pair: Option<SelectedDirectPairV1>,
    terminal_receipt_id: [u8; 32],
    rent: DirectRentOwnerV1,
    phase: DirectSelectionPhaseV1,
}

impl DirectSelectionV1 {
    /// Exact Selection account.
    pub const fn account(self) -> [u8; 32] { self.selection_account }
    /// Exact number of complete live Reservation records.
    pub const fn reservation_count(self) -> u8 { self.reservation_count }
    /// Frozen owner-blind RelationV2 domain.
    pub const fn domain(&self) -> &EconomicDomainV2 { &self.domain }
    /// Frozen owner-blind RelationV2 book.
    pub const fn book(&self) -> &DirectEconomicBookV1 { &self.book }
    /// Frozen exact price precondition.
    pub const fn price(&self) -> &PricePreconditionV2 { &self.price }
    /// Number of retained valid submissions.
    pub const fn candidate_count(self) -> u8 { self.candidate_count }
    /// Next canonical verification coordinate.
    pub const fn verification_cursor(self) -> u8 { self.verification_cursor }
    /// Current lifecycle.
    pub const fn phase(self) -> DirectSelectionPhaseV1 { self.phase }
    /// Persisted rent ownership.
    pub const fn rent(self) -> DirectRentOwnerV1 { self.rent }
    /// Complete selected pair capability, present only after action 8.
    pub const fn selected_pair(self) -> Option<SelectedDirectPairV1> { self.selected_pair }
    /// Economic terminal receipt, nonzero only after action 9..12.
    pub const fn terminal_receipt_id(self) -> [u8; 32] { self.terminal_receipt_id }

    /// Exact Reservation account at one canonical book coordinate.
    pub fn reservation_account(self, index: u8) -> Result<[u8; 32], DirectMarketErrorV1> {
        let at = usize::from(index);
        if at >= usize::from(self.reservation_count) {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        Ok(self.reservation_accounts[at])
    }

    /// Exact Reservation semantic ID at one canonical book coordinate.
    pub fn reservation_semantic_id(
        self,
        index: u8,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        let at = usize::from(index);
        if at >= usize::from(self.reservation_count) {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        Ok(self.reservation_semantic_ids[at])
    }

    /// Retained candidate at one canonical submission coordinate.
    pub fn candidate(self, index: u8) -> Result<DirectEconomicCandidateV1, DirectMarketErrorV1> {
        let at = usize::from(index);
        if at >= usize::from(self.candidate_count) {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        Ok(self.candidates[at])
    }

    /// Validate canonical padding, RelationV2 inputs, submissions, and phase.
    pub fn validate_against(self, root: DirectMarketRootV1) -> Result<(), DirectMarketErrorV1> {
        root.validate()?;
        self.rent.validate()?;
        for id in [self.market_instance_id, self.direct_root_account, self.selection_account] {
            require_live(id)?;
        }
        let binding = root.binding();
        if self.market_instance_id != binding.market_instance_id
            || self.generation != binding.generation
            || self.direct_root_account != binding.direct_root_account
            || self.selection_account != root.selection_account()
            || self.reservation_count != self.book.len
            || usize::from(self.reservation_count) > 2
            || usize::from(self.candidate_count) > CANDIDATE_CAPACITY_V1
            || self.verification_cursor > self.candidate_count
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        validate_domain_binding(root, &self.domain, &self.price)?;
        self.book.validate(&self.domain)?;
        let mut index = 0usize;
        while index < 2 {
            if index < usize::from(self.reservation_count) {
                require_live(self.reservation_accounts[index])?;
                require_live(self.reservation_semantic_ids[index])?;
                if self.book.orders[index].order_id == [0; 32]
                    || (index != 0
                        && self.reservation_accounts[index - 1] == self.reservation_accounts[index])
                {
                    return Err(DirectMarketErrorV1::IdentityAlias);
                }
            } else if self.reservation_accounts[index] != [0; 32]
                || self.reservation_semantic_ids[index] != [0; 32]
            {
                return Err(DirectMarketErrorV1::InvalidCount);
            }
            index += 1;
        }
        index = 0;
        while index < CANDIDATE_CAPACITY_V1 {
            if index < usize::from(self.candidate_count) {
                let economics = verify_compact_direct_candidate_v1(
                    &self.domain,
                    &self.book,
                    &self.price,
                    self.candidates[index],
                )?;
                if economics.economic_candidate_digest != self.candidate_digests[index] {
                    return Err(DirectMarketErrorV1::MismatchedBinding);
                }
                let mut previous = 0usize;
                while previous < index {
                    if self.candidate_digests[previous] == self.candidate_digests[index] {
                        return Err(DirectMarketErrorV1::IdentityAlias);
                    }
                    previous += 1;
                }
            } else if self.candidates[index] != DirectEconomicCandidateV1::EMPTY
                || self.candidate_digests[index] != [0; 32]
            {
                return Err(DirectMarketErrorV1::InvalidCount);
            }
            index += 1;
        }
        let verified_prefix = low_mask(self.verification_cursor)?;
        if self.verified_mask != verified_prefix {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        match (self.phase, root.phase()) {
            (DirectSelectionPhaseV1::FrozenEmpty, DirectRootPhaseV1::FrozenEmpty)
                if self.reservation_count < MAX_DIRECT_RESERVATIONS_V1
                    && self.candidate_count == 0
                    && self.verification_cursor == 0
                    && self.selected_pair.is_none()
                    && self.selected_candidate_index.is_none()
                    && self.terminal_receipt_id == [0; 32] => {}
            (DirectSelectionPhaseV1::SubmissionOpen, DirectRootPhaseV1::SubmissionOpen)
                if self.reservation_count == MAX_DIRECT_RESERVATIONS_V1
                    && self.verification_cursor == 0
                    && self.selected_pair.is_none()
                    && self.selected_candidate_index.is_none()
                    && self.terminal_receipt_id == [0; 32] => {}
            (DirectSelectionPhaseV1::Verifying, DirectRootPhaseV1::Verifying)
                if self.selected_pair.is_none()
                    && self.selected_candidate_index.is_none()
                    && self.terminal_receipt_id == [0; 32] => {}
            (DirectSelectionPhaseV1::Selected, DirectRootPhaseV1::Selected)
                if self.candidate_count != 0
                    && self.verification_cursor == self.candidate_count
                    && self.selected_pair.is_some()
                    && self.selected_candidate_index.is_some()
                    && self.terminal_receipt_id == [0; 32] => {
                        self.validate_selected_pair()?;
                    }
            (DirectSelectionPhaseV1::Terminal, DirectRootPhaseV1::Terminal)
                if self.terminal_receipt_id != [0; 32] => {
                    require_live(self.terminal_receipt_id)?;
                }
            _ => return Err(DirectMarketErrorV1::WrongPhase),
        }
        require_live(self.traversal_transcript_id)?;
        Ok(())
    }

    /// Domain-separated identity of the complete Selection state.
    pub fn semantic_id<B: DirectHashBackendV1>(
        self,
        root: DirectMarketRootV1,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate_against(root)?;
        let book_id = book_id(&self.book, backend)?;
        let selected_index = self.selected_candidate_index.map_or(u8::MAX, |value| value);
        let selected_transcript = self
            .selected_pair
            .map_or([0u8; 253], |pair| pair.canonical_transcript());
        let id = backend.sha256_parts(&[
            SELECTION_STATE_DOMAIN_V1,
            &self.market_instance_id,
            &self.generation.to_le_bytes(),
            &self.direct_root_account,
            &self.selection_account,
            &[self.reservation_count],
            &self.reservation_accounts[0],
            &self.reservation_accounts[1],
            &self.reservation_semantic_ids[0],
            &self.reservation_semantic_ids[1],
            &book_id,
            &self.domain.relation_version.to_le_bytes(),
            &self.domain.market_semantics_digest,
            &self.domain.epoch_semantics_digest,
            &self.domain.relation_policy_digest,
            &self.domain.price_policy_digest,
            &self.domain.epoch_index.to_le_bytes(),
            &[self.domain.outcome_count],
            &self.domain.price_scale.to_le_bytes(),
            &self.price.semantic_price_digest,
            &[self.candidate_count],
            &self.candidate_digests[0],
            &self.candidate_digests[1],
            &self.candidate_digests[2],
            &[self.verification_cursor],
            &[self.verified_mask],
            &self.traversal_transcript_id,
            &[selected_index],
            &selected_transcript,
            &self.terminal_receipt_id,
            &self.rent.payer,
            &self.rent.principal_lamports.to_le_bytes(),
            &self.rent.donation_floor_lamports.to_le_bytes(),
            &[self.phase.byte()],
        ]);
        require_live(id)?;
        Ok(id)
    }

    fn validate_selected_pair(self) -> Result<(), DirectMarketErrorV1> {
        let index = self
            .selected_candidate_index
            .ok_or(DirectMarketErrorV1::InvalidCount)?;
        let pair = self.selected_pair.ok_or(DirectMarketErrorV1::InvalidCount)?;
        if usize::from(index) >= usize::from(self.candidate_count)
            || pair.selection_transcript_id() != self.traversal_transcript_id
            || pair.economic_candidate_digest() != self.candidate_digests[usize::from(index)]
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        Ok(())
    }

    pub(crate) fn terminalize(
        mut self,
        root_post: DirectMarketRootV1,
        receipt: [u8; 32],
    ) -> Result<Self, DirectMarketErrorV1> {
        if self.phase == DirectSelectionPhaseV1::Terminal {
            return Err(DirectMarketErrorV1::WrongPhase);
        }
        require_live(receipt)?;
        self.phase = DirectSelectionPhaseV1::Terminal;
        self.terminal_receipt_id = receipt;
        self.validate_against(root_post)?;
        Ok(self)
    }
}

/// Atomic root/replay and fresh Selection poststate for action 4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectSelectionFreezePlanV1 {
    /// Root and permanent replay after exact freeze.
    pub state: DirectRootReplayPostV1,
    /// Fresh Selection owning the complete frozen prefix.
    pub selection: DirectSelectionV1,
}

/// Build the complete canonical Reservation prefix and freeze action 4.
#[allow(clippy::too_many_arguments)]
pub fn prepare_direct_selection_freeze_v1<
    A: AuthenticatedDirectSelectionFreezeV1 + ?Sized,
    B: DirectHashBackendV1,
>(
    authority: &A,
    state: DirectRootReplayPostV1,
    consumed_sequence: u64,
    observed_slot: u64,
    selection_account: [u8; 32],
    rent: DirectRentOwnerV1,
    reservations: [Option<DirectReservationV1>; 2],
    domain: EconomicDomainV2,
    price: PricePreconditionV2,
    backend: &B,
) -> Result<DirectSelectionFreezePlanV1, DirectMarketErrorV1> {
    state.replay.validate_against(state.root)?;
    require_fresh_child_account(state.root.binding(), selection_account)?;
    rent.validate()?;
    validate_domain_binding(state.root, &domain, &price)?;
    let (ordered, reservation_count) = canonical_reservation_prefix(state.root, reservations)?;
    let mut reservation_accounts = [[0u8; 32]; 2];
    let mut reservation_semantic_ids = [[0u8; 32]; 2];
    let mut book = DirectEconomicBookV1 {
        orders: [EMPTY_ECONOMIC_ORDER_V2; 2],
        len: 0,
    };
    let mut index = 0usize;
    while index < usize::from(reservation_count) {
        let reservation = ordered[index].ok_or(DirectMarketErrorV1::InvalidCount)?;
        if reservation.account() == selection_account {
            return Err(DirectMarketErrorV1::IdentityAlias);
        }
        reservation_accounts[index] = reservation.account();
        reservation_semantic_ids[index] = reservation.semantic_id(backend)?;
        book.orders[index] = reservation.economic_order()?;
        index += 1;
    }
    book.len = reservation_count;
    book.validate(&domain)?;
    if reservation_count == MAX_DIRECT_RESERVATIONS_V1 {
        validate_complete_pair(&ordered)?;
    }
    authority.authenticate_freeze(
        state.root,
        selection_account,
        rent,
        &ordered,
        &reservation_semantic_ids,
        &domain,
        &price,
    )?;
    let phase = if reservation_count == MAX_DIRECT_RESERVATIONS_V1 {
        DirectSelectionPhaseV1::SubmissionOpen
    } else {
        DirectSelectionPhaseV1::FrozenEmpty
    };
    let traversal_transcript_id = backend.sha256_parts(&[
        SELECTION_TRAVERSAL_DOMAIN_V1,
        &state.root.binding().market_instance_id,
        &state.root.binding().generation.to_le_bytes(),
        &selection_account,
        &[reservation_count],
        &reservation_semantic_ids[0],
        &reservation_semantic_ids[1],
        &price.semantic_price_digest,
    ]);
    require_live(traversal_transcript_id)?;
    let selection_pre_root = DirectSelectionV1 {
        market_instance_id: state.root.binding().market_instance_id,
        generation: state.root.binding().generation,
        direct_root_account: state.root.binding().direct_root_account,
        selection_account,
        reservation_accounts,
        reservation_semantic_ids,
        reservation_count,
        domain,
        book,
        price,
        candidates: [DirectEconomicCandidateV1::EMPTY; CANDIDATE_CAPACITY_V1],
        candidate_digests: [[0; 32]; CANDIDATE_CAPACITY_V1],
        candidate_count: 0,
        verification_cursor: 0,
        verified_mask: 0,
        traversal_transcript_id,
        selected_candidate_index: None,
        selected_pair: None,
        terminal_receipt_id: [0; 32],
        rent,
        phase,
    };
    let mut expected_root = state.root;
    expected_root.selection_account = selection_account;
    expected_root.phase = if reservation_count == MAX_DIRECT_RESERVATIONS_V1 {
        DirectRootPhaseV1::SubmissionOpen
    } else {
        DirectRootPhaseV1::FrozenEmpty
    };
    selection_pre_root.validate_against(expected_root)?;
    let selection_poststate_id = selection_pre_root.semantic_id(expected_root, backend)?;
    let state = state.freeze(
        consumed_sequence,
        observed_slot,
        selection_account,
        selection_poststate_id,
        backend,
    )?;
    Ok(DirectSelectionFreezePlanV1 { state, selection: selection_pre_root })
}

/// Submit one checked, nonduplicate candidate under action 5.
pub fn submit_direct_candidate_v1<B: DirectHashBackendV1>(
    state: DirectRootReplayPostV1,
    mut selection: DirectSelectionV1,
    consumed_sequence: u64,
    observed_slot: u64,
    candidate: DirectEconomicCandidateV1,
    backend: &B,
) -> Result<DirectSelectionFreezePlanV1, DirectMarketErrorV1> {
    selection.validate_against(state.root)?;
    if selection.phase != DirectSelectionPhaseV1::SubmissionOpen
        || usize::from(selection.candidate_count) >= CANDIDATE_CAPACITY_V1
    {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    let economics = verify_compact_direct_candidate_v1(
        &selection.domain,
        &selection.book,
        &selection.price,
        candidate,
    )?;
    let mut index = 0usize;
    while index < usize::from(selection.candidate_count) {
        if selection.candidate_digests[index] == economics.economic_candidate_digest {
            return Err(DirectMarketErrorV1::IdentityAlias);
        }
        index += 1;
    }
    let at = usize::from(selection.candidate_count);
    selection.candidates[at] = candidate;
    selection.candidate_digests[at] = economics.economic_candidate_digest;
    selection.candidate_count = selection
        .candidate_count
        .checked_add(1)
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    let selection_poststate_id = selection.semantic_id(state.root, backend)?;
    let state = state.record_submission(
        consumed_sequence,
        observed_slot,
        selection_poststate_id,
        backend,
    )?;
    Ok(DirectSelectionFreezePlanV1 { state, selection })
}

/// Begin action 6's exhaustive submission-order traversal.
pub fn begin_direct_candidate_verification_v1<B: DirectHashBackendV1>(
    state: DirectRootReplayPostV1,
    mut selection: DirectSelectionV1,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectSelectionFreezePlanV1, DirectMarketErrorV1> {
    selection.validate_against(state.root)?;
    if selection.phase != DirectSelectionPhaseV1::SubmissionOpen {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    selection.phase = DirectSelectionPhaseV1::Verifying;
    let mut root_projection = state.root;
    root_projection.phase = DirectRootPhaseV1::Verifying;
    let selection_poststate_id = selection.semantic_id(root_projection, backend)?;
    let state = state.begin_verification(
        consumed_sequence,
        observed_slot,
        selection_poststate_id,
        backend,
    )?;
    Ok(DirectSelectionFreezePlanV1 { state, selection })
}

/// Reverify exactly the next retained candidate under action 7.
pub fn verify_next_direct_candidate_v1<B: DirectHashBackendV1>(
    state: DirectRootReplayPostV1,
    mut selection: DirectSelectionV1,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectSelectionFreezePlanV1, DirectMarketErrorV1> {
    selection.validate_against(state.root)?;
    if selection.phase != DirectSelectionPhaseV1::Verifying
        || selection.verification_cursor >= selection.candidate_count
    {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    let at = usize::from(selection.verification_cursor);
    let economics = verify_compact_direct_candidate_v1(
        &selection.domain,
        &selection.book,
        &selection.price,
        selection.candidates[at],
    )?;
    if economics.economic_candidate_digest != selection.candidate_digests[at] {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    let verified_bit = bit_for(selection.verification_cursor)?;
    selection.verified_mask |= verified_bit;
    selection.traversal_transcript_id = backend.sha256_parts(&[
        SELECTION_TRAVERSAL_DOMAIN_V1,
        &selection.market_instance_id,
        &selection.generation.to_le_bytes(),
        &selection.selection_account,
        &[selection.verification_cursor],
        &selection.candidate_digests[at],
        &selection.traversal_transcript_id,
    ]);
    require_live(selection.traversal_transcript_id)?;
    selection.verification_cursor = selection
        .verification_cursor
        .checked_add(1)
        .ok_or(DirectMarketErrorV1::Arithmetic)?;
    let selection_poststate_id = selection.semantic_id(state.root, backend)?;
    let state = state.record_verification(
        consumed_sequence,
        observed_slot,
        selection_poststate_id,
        backend,
    )?;
    Ok(DirectSelectionFreezePlanV1 { state, selection })
}

/// Finalize action 8 and mint the private exact selected-pair capability.
pub fn finalize_direct_selection_v1<B: DirectHashBackendV1>(
    state: DirectRootReplayPostV1,
    mut selection: DirectSelectionV1,
    consumed_sequence: u64,
    observed_slot: u64,
    backend: &B,
) -> Result<DirectSelectionFreezePlanV1, DirectMarketErrorV1> {
    selection.validate_against(state.root)?;
    if selection.phase != DirectSelectionPhaseV1::Verifying
        || selection.candidate_count == 0
        || selection.verification_cursor != selection.candidate_count
    {
        return Err(DirectMarketErrorV1::WrongPhase);
    }
    let mut best_index = 0usize;
    let mut best_economics = verify_compact_direct_candidate_v1(
        &selection.domain,
        &selection.book,
        &selection.price,
        selection.candidates[0],
    )?;
    let mut index = 1usize;
    while index < usize::from(selection.candidate_count) {
        let economics = verify_compact_direct_candidate_v1(
            &selection.domain,
            &selection.book,
            &selection.price,
            selection.candidates[index],
        )?;
        if economics
            .score
            .total_order_same_domain(&best_economics.score)
            .map_err(|error| {
                DirectMarketErrorV1::Economic(
                    clutch_batch::relation_v2::EconomicErrorV2::Score(error),
                )
            })?
            == Ordering::Greater
        {
            best_index = index;
            best_economics = economics;
        }
        index += 1;
    }
    let selected = selection.candidates[best_index];
    let selected_index =
        u8::try_from(best_index).map_err(|_| DirectMarketErrorV1::Arithmetic)?;
    let selected_traversal_id = backend.sha256_parts(&[
        SELECTED_TRAVERSAL_DOMAIN_V1,
        &selection.market_instance_id,
        &selection.generation.to_le_bytes(),
        &selection.selection_account,
        &[selection.candidate_count],
        &selection.traversal_transcript_id,
        &[selected_index],
        &selection.candidate_digests[usize::from(selected_index)],
    ]);
    require_live(selected_traversal_id)?;
    let authority = FrozenSelectionAuthorityV1 {
        selected_traversal_id,
        expected_candidate_digest: selection.candidate_digests[usize::from(selected_index)],
        expected_price_digest: selection.price.semantic_price_digest,
    };
    let pair = authenticate_compact_selected_direct_pair_v1(
        &authority,
        selected_traversal_id,
        &selection.domain,
        &selection.book,
        &selection.price,
        selected,
    )?;
    selection.traversal_transcript_id = selected_traversal_id;
    selection.selected_candidate_index = Some(selected_index);
    selection.selected_pair = Some(pair);
    selection.phase = DirectSelectionPhaseV1::Selected;
    let state = state.select(
        consumed_sequence,
        observed_slot,
        selected_traversal_id,
        backend,
    )?;
    selection.validate_against(state.root)?;
    Ok(DirectSelectionFreezePlanV1 { state, selection })
}

#[derive(Clone, Copy, Debug)]
struct FrozenSelectionAuthorityV1 {
    selected_traversal_id: [u8; 32],
    expected_candidate_digest: [u8; 32],
    expected_price_digest: [u8; 32],
}

impl AuthenticatedDirectSelectionAuthorityV1 for FrozenSelectionAuthorityV1 {
    fn authenticate_compact_selected_pair(
        &self,
        selection_transcript_id: [u8; 32],
        _domain: &EconomicDomainV2,
        _orders: &[EconomicOrderV2; 2],
        price: &PricePreconditionV2,
        _candidate: DirectEconomicCandidateV1,
        economics: &VerifiedEconomicsV2,
    ) -> Result<(), DirectPairErrorV1> {
        if selection_transcript_id == self.selected_traversal_id
            && economics.economic_candidate_digest == self.expected_candidate_digest
            && price.semantic_price_digest == self.expected_price_digest
        {
            Ok(())
        } else {
            Err(DirectPairErrorV1::UnauthenticatedSelection)
        }
    }
}

fn canonical_reservation_prefix(
    root: DirectMarketRootV1,
    reservations: [Option<DirectReservationV1>; 2],
) -> Result<([Option<DirectReservationV1>; 2], u8), DirectMarketErrorV1> {
    let mut values = reservations;
    let count = match values {
        [None, None] => 0,
        [Some(_), None] => 1,
        [None, Some(value)] => {
            values = [Some(value), None];
            1
        }
        [Some(left), Some(right)] => {
            if left.account() == right.account() || left.order_id() == right.order_id() {
                return Err(DirectMarketErrorV1::IdentityAlias);
            }
            if right.order_id() < left.order_id() {
                values = [Some(right), Some(left)];
            }
            2
        }
    };
    let count = u8::try_from(count).map_err(|_| DirectMarketErrorV1::Arithmetic)?;
    if count != root.live_reservations() {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut index = 0usize;
    while index < usize::from(count) {
        let reservation = values[index].ok_or(DirectMarketErrorV1::InvalidCount)?;
        reservation.validate()?;
        let binding = root.binding();
        if reservation.phase() != DirectReservationPhaseV1::Active
            || reservation.market_instance_id != binding.market_instance_id
            || reservation.generation != binding.generation
            || reservation.direct_root_account != binding.direct_root_account
            || reservation.general_market_runtime != binding.general_market_runtime
            || reservation.outcome_count != binding.outcome_count
            || reservation.price_scale != binding.price_scale
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        index += 1;
    }
    Ok((values, count))
}

fn validate_complete_pair(
    reservations: &[Option<DirectReservationV1>; 2],
) -> Result<(), DirectMarketErrorV1> {
    let left = reservations[0].ok_or(DirectMarketErrorV1::InvalidCount)?;
    let right = reservations[1].ok_or(DirectMarketErrorV1::InvalidCount)?;
    if left.side() == right.side() || left.outcome() != right.outcome() {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    Ok(())
}

fn validate_domain_binding(
    root: DirectMarketRootV1,
    domain: &EconomicDomainV2,
    price: &PricePreconditionV2,
) -> Result<(), DirectMarketErrorV1> {
    let binding = root.binding();
    if domain.relation_version != ECONOMIC_RELATION_VERSION_V2
        || domain.market_semantics_digest != binding.market_instance_id
        || domain.epoch_semantics_digest != binding.resolution_semantic_id
        || domain.relation_policy_digest != binding.relation_policy_id
        || domain.price_policy_digest != binding.price_policy_id
        || domain.epoch_index != binding.generation
        || domain.outcome_count != binding.outcome_count
        || domain.price_scale != binding.price_scale
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    domain.validate()?;
    price.validate(domain)?;
    Ok(())
}

fn book_id<B: DirectHashBackendV1>(
    book: &DirectEconomicBookV1,
    backend: &B,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    if usize::from(book.len) > 2 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let first = order_id(book.orders[0], backend)?;
    let second = order_id(book.orders[1], backend)?;
    let id = backend.sha256_parts(&[SELECTION_BOOK_DOMAIN_V1, &[book.len], &first, &second]);
    require_live(id)?;
    Ok(id)
}

fn order_id<B: DirectHashBackendV1>(
    order: EconomicOrderV2,
    backend: &B,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    if order == EMPTY_ECONOMIC_ORDER_V2 {
        return Ok([0; 32]);
    }
    let mut coefficients = [0u8; 16 * 8];
    let mut outcome = 0usize;
    while outcome < 16 {
        let start = outcome.checked_mul(8).ok_or(DirectMarketErrorV1::Arithmetic)?;
        let end = start.checked_add(8).ok_or(DirectMarketErrorV1::Arithmetic)?;
        coefficients[start..end].copy_from_slice(&order.coefficients[outcome].to_le_bytes());
        outcome += 1;
    }
    let id = backend.sha256_parts(&[
        SELECTION_ORDER_DOMAIN_V1,
        &order.order_id,
        &[side_byte(order.side)],
        &coefficients,
        &order.quantity.to_le_bytes(),
        &order.minimum_fill.to_le_bytes(),
        &[partial_policy_byte(order.partial_policy)],
        &order.expiry_epoch.to_le_bytes(),
        &order.limit_value_price_units_per_unit.to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(id)
}

fn low_mask(count: u8) -> Result<u8, DirectMarketErrorV1> {
    if usize::from(count) > CANDIDATE_CAPACITY_V1 {
        return Err(DirectMarketErrorV1::InvalidCount);
    }
    let mut mask = 0u8;
    let mut index = 0u8;
    while index < count {
        mask |= bit_for(index)?;
        index = index.checked_add(1).ok_or(DirectMarketErrorV1::Arithmetic)?;
    }
    Ok(mask)
}

fn bit_for(index: u8) -> Result<u8, DirectMarketErrorV1> {
    1u8.checked_shl(u32::from(index)).ok_or(DirectMarketErrorV1::Arithmetic)
}

const fn side_byte(side: Side) -> u8 {
    match side { Side::Buy => 1, Side::Sell => 2 }
}

const fn partial_policy_byte(policy: clutch_batch::PartialPolicy) -> u8 {
    match policy {
        clutch_batch::PartialPolicy::Allow => 1,
        clutch_batch::PartialPolicy::AllOrNone => 2,
    }
}
