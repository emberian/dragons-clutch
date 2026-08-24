// SPDX-License-Identifier: AGPL-3.0-or-later
//! Direct's bounded consumer of the shared seven-account liveness runtime.
//!
//! Direct never mints a lone Candidate compartment. Product must first prove
//! atomic capitalization of the complete canonical runtime bundle and then
//! allocate a disjoint Candidate call range to one Direct occurrence. The
//! complete seven-row transcript is streamed so neither the kernel nor SBF
//! adapters need to copy a full bundle into one call frame.

use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RUNTIME_COMPARTMENT_COUNT_V1,
    RUNTIME_COMPARTMENT_ORDER_V1,
};

use crate::{require_live, DirectHashBackendV1, DirectMarketErrorV1};

const DIRECT_CANDIDATE_WORK_SCHEDULE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/candidate-work-schedule/v1\0";
const DIRECT_GLOBAL_LIVENESS_ROW_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/global-liveness-row/v1\0";
const DIRECT_GLOBAL_LIVENESS_BUNDLE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/direct/global-liveness-bundle/v1\0";

/// Exact number of protocol-funded calls reserved for a complete Direct path.
///
/// Candidate submission is voluntary caller work. The retained top three each
/// carry separate Direct-owned verification bonds; this schedule capitalizes
/// the keeper-required freeze, traversal, terminal, and retirement calls.
pub const DIRECT_CANDIDATE_RESERVED_CALLS_V1: u32 = 8;

/// Direct-owned quote preimage for its bounded share of Candidate work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCandidateWorkScheduleV1 {
    /// Maximum keeper payment for action 4.
    pub freeze_book_lamports: u64,
    /// Maximum keeper payment for action 6.
    pub begin_verification_lamports: u64,
    /// Maximum keeper payment for each of at most three action 7 calls.
    pub verify_candidate_lamports: u64,
    /// Maximum keeper payment for action 8.
    pub finalize_selection_lamports: u64,
    /// Maximum keeper payment for exactly one of actions 9..=12.
    pub economic_terminal_lamports: u64,
    /// Maximum keeper payment for action 13.
    pub retire_terminal_lamports: u64,
    /// Exact refundable principal posted by every currently retained candidate.
    pub retained_candidate_bond_lamports: u64,
}

impl DirectCandidateWorkScheduleV1 {
    /// Require a nonzero ceiling and a nonzero anti-grind bond for every role.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        for amount in [
            self.freeze_book_lamports,
            self.begin_verification_lamports,
            self.verify_candidate_lamports,
            self.finalize_selection_lamports,
            self.economic_terminal_lamports,
            self.retire_terminal_lamports,
            self.retained_candidate_bond_lamports,
        ] {
            if amount == 0 {
                return Err(DirectMarketErrorV1::InvalidCount);
            }
        }
        self.reserved_work_lamports()?;
        Ok(())
    }

    /// Exact prepaid work allocated to the eight keeper-required calls.
    pub fn reserved_work_lamports(self) -> Result<u64, DirectMarketErrorV1> {
        self.validate_nonrecursive()?;
        let verification = self
            .verify_candidate_lamports
            .checked_mul(3)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        self.freeze_book_lamports
            .checked_add(self.begin_verification_lamports)
            .and_then(|value| value.checked_add(verification))
            .and_then(|value| value.checked_add(self.finalize_selection_lamports))
            .and_then(|value| value.checked_add(self.economic_terminal_lamports))
            .and_then(|value| value.checked_add(self.retire_terminal_lamports))
            .ok_or(DirectMarketErrorV1::Arithmetic)
    }

    fn validate_nonrecursive(self) -> Result<(), DirectMarketErrorV1> {
        if self.freeze_book_lamports == 0
            || self.begin_verification_lamports == 0
            || self.verify_candidate_lamports == 0
            || self.finalize_selection_lamports == 0
            || self.economic_terminal_lamports == 0
            || self.retire_terminal_lamports == 0
            || self.retained_candidate_bond_lamports == 0
        {
            Err(DirectMarketErrorV1::InvalidCount)
        } else {
            Ok(())
        }
    }

    /// Largest single call ceiling, bounded by the shared Candidate owner.
    pub fn maximum_lamports_per_call(self) -> Result<u64, DirectMarketErrorV1> {
        self.validate_nonrecursive()?;
        let mut maximum = self.freeze_book_lamports;
        for value in [
            self.begin_verification_lamports,
            self.verify_candidate_lamports,
            self.finalize_selection_lamports,
            self.economic_terminal_lamports,
            self.retire_terminal_lamports,
        ] {
            if value > maximum {
                maximum = value;
            }
        }
        Ok(maximum)
    }

    /// Canonical identity of this exact occurrence-specific quote schedule.
    #[allow(clippy::too_many_arguments)]
    pub fn semantic_id<B: DirectHashBackendV1>(
        self,
        market_instance_id: [u8; 32],
        generation: u64,
        direct_root_account: [u8; 32],
        family_admission_sequence: u32,
        candidate_lifecycle_policy_id: [u8; 32],
        candidate_liveness_policy_id: [u8; 32],
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        for identity in [
            market_instance_id,
            direct_root_account,
            candidate_lifecycle_policy_id,
            candidate_liveness_policy_id,
        ] {
            require_live(identity)?;
        }
        if generation == 0 {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        let id = backend.sha256_parts(&[
            DIRECT_CANDIDATE_WORK_SCHEDULE_DOMAIN_V1,
            &market_instance_id,
            &generation.to_le_bytes(),
            &direct_root_account,
            &family_admission_sequence.to_le_bytes(),
            &candidate_lifecycle_policy_id,
            &candidate_liveness_policy_id,
            &DIRECT_CANDIDATE_RESERVED_CALLS_V1.to_le_bytes(),
            &self.freeze_book_lamports.to_le_bytes(),
            &self.begin_verification_lamports.to_le_bytes(),
            &self.verify_candidate_lamports.to_le_bytes(),
            &self.finalize_selection_lamports.to_le_bytes(),
            &self.economic_terminal_lamports.to_le_bytes(),
            &self.retire_terminal_lamports.to_le_bytes(),
            &self.retained_candidate_bond_lamports.to_le_bytes(),
        ]);
        require_live(id)?;
        Ok(id)
    }
}

/// One streamed, hostile-authenticated global runtime compartment record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectGlobalLivenessCompartmentV1 {
    /// Canonical compartment kind at this row.
    pub kind: RuntimeCompartmentKindV1,
    /// Exact persisted account.
    pub account_id: [u8; 32],
    /// Exact hostile account-data identity at Product allocation.
    pub account_data_id: [u8; 32],
    /// Exact atomic capitalization receipt for this physical account.
    pub capitalization_receipt_id: [u8; 32],
    /// Shared runtime semantic owner.
    pub semantic_owner: [u8; 32],
    /// Shared runtime quote schedule.
    pub quote_schedule_id: [u8; 32],
    /// Program authorized to own work receipts.
    pub receipt_program_id: [u8; 32],
    /// Stable physical lifecycle generation.
    pub generation: u64,
}

impl DirectGlobalLivenessCompartmentV1 {
    fn validate(self) -> Result<(), DirectMarketErrorV1> {
        for identity in [
            self.account_id,
            self.account_data_id,
            self.capitalization_receipt_id,
            self.semantic_owner,
            self.quote_schedule_id,
            self.receipt_program_id,
        ] {
            require_live(identity)?;
        }
        if self.generation == 0 {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        Ok(())
    }
}

/// Allocation-free canonical stream over all seven global runtime rows.
pub trait DirectGlobalLivenessBundleStreamV1 {
    /// Return one exact canonical row. Default implementations cannot mint it.
    fn compartment(
        &self,
        _kind: RuntimeCompartmentKindV1,
    ) -> Result<DirectGlobalLivenessCompartmentV1, DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Hash the complete seven-row global capitalization transcript.
pub fn direct_global_liveness_bundle_commitment_v1<
    S: DirectGlobalLivenessBundleStreamV1 + ?Sized,
    B: DirectHashBackendV1,
>(
    stream: &S,
    policy_account: [u8; 32],
    policy_id: [u8; 32],
    policy_data_id: [u8; 32],
    global_lifecycle_id: [u8; 32],
    global_bundle_binding_id: [u8; 32],
    global_capitalization_receipt_id: [u8; 32],
    backend: &B,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    for identity in [
        policy_account,
        policy_id,
        policy_data_id,
        global_lifecycle_id,
        global_bundle_binding_id,
        global_capitalization_receipt_id,
    ] {
        require_live(identity)?;
    }
    let mut row_ids = [[0u8; 32]; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut accounts = [[0u8; 32]; RUNTIME_COMPARTMENT_COUNT_V1];
    let mut index = 0usize;
    while index < RUNTIME_COMPARTMENT_COUNT_V1 {
        let expected_kind = RUNTIME_COMPARTMENT_ORDER_V1[index];
        let row = stream.compartment(expected_kind)?;
        row.validate()?;
        if row.kind != expected_kind {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        let mut prior = 0usize;
        while prior < index {
            if accounts[prior] == row.account_id {
                return Err(DirectMarketErrorV1::IdentityAlias);
            }
            prior += 1;
        }
        accounts[index] = row.account_id;
        row_ids[index] = backend.sha256_parts(&[
            DIRECT_GLOBAL_LIVENESS_ROW_DOMAIN_V1,
            &[runtime_compartment_byte_v1(row.kind)],
            &row.account_id,
            &row.account_data_id,
            &row.capitalization_receipt_id,
            &row.semantic_owner,
            &row.quote_schedule_id,
            &row.receipt_program_id,
            &row.generation.to_le_bytes(),
        ]);
        require_live(row_ids[index])?;
        index += 1;
    }
    let id = backend.sha256_parts(&[
        DIRECT_GLOBAL_LIVENESS_BUNDLE_DOMAIN_V1,
        &policy_account,
        &policy_id,
        &policy_data_id,
        &global_lifecycle_id,
        &global_bundle_binding_id,
        &global_capitalization_receipt_id,
        &row_ids[0],
        &row_ids[1],
        &row_ids[2],
        &row_ids[3],
        &row_ids[4],
        &row_ids[5],
        &row_ids[6],
    ]);
    require_live(id)?;
    Ok(id)
}

/// Exact Product allocation persisted transitively by the Direct root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectCandidateLivenessBindingV1 {
    /// Shared policy account.
    pub policy_account: [u8; 32],
    /// Hostile policy account-data identity.
    pub policy_data_id: [u8; 32],
    /// Shared global Market lifecycle identity.
    pub global_lifecycle_id: [u8; 32],
    /// Exact binding of all seven physical runtime accounts.
    pub global_bundle_binding_id: [u8; 32],
    /// Root receipt proving atomic present capitalization of all seven rows.
    pub global_capitalization_receipt_id: [u8; 32],
    /// Complete canonical seven-row transcript commitment.
    pub global_bundle_commitment_id: [u8; 32],
    /// Candidate row account allocated to Direct.
    pub candidate_account: [u8; 32],
    /// Candidate hostile pre-allocation account-data identity.
    pub candidate_data_id: [u8; 32],
    /// Shared semantic owner which authorizes its work receipts.
    pub candidate_semantic_owner: [u8; 32],
    /// Shared runtime quote schedule for this physical compartment.
    pub candidate_quote_schedule_id: [u8; 32],
    /// Program which owns the Candidate work receipts.
    pub candidate_receipt_program_id: [u8; 32],
    /// Stable Candidate account generation.
    pub candidate_generation: u64,
    /// First one-based shared call ordinal reserved by Product.
    pub first_call_ordinal: u32,
    /// Exact number of disjoint reserved calls.
    pub reserved_calls: u32,
    /// Exact prepaid work reserved for this Direct occurrence.
    pub reserved_work_lamports: u64,
    /// Product-owned one-way allocation receipt.
    pub allocation_receipt_id: [u8; 32],
    /// Direct-owned per-action quote preimage.
    pub work_schedule: DirectCandidateWorkScheduleV1,
    /// Identity of the exact Direct quote preimage.
    pub work_schedule_id: [u8; 32],
}

impl DirectCandidateLivenessBindingV1 {
    /// Validate all identities, the finite range, and exact work sum.
    pub fn validate(self) -> Result<(), DirectMarketErrorV1> {
        for identity in [
            self.policy_account,
            self.policy_data_id,
            self.global_lifecycle_id,
            self.global_bundle_binding_id,
            self.global_capitalization_receipt_id,
            self.global_bundle_commitment_id,
            self.candidate_account,
            self.candidate_data_id,
            self.candidate_semantic_owner,
            self.candidate_quote_schedule_id,
            self.candidate_receipt_program_id,
            self.allocation_receipt_id,
            self.work_schedule_id,
        ] {
            require_live(identity)?;
        }
        self.work_schedule.validate()?;
        if self.candidate_generation == 0
            || self.first_call_ordinal == 0
            || self.reserved_calls != DIRECT_CANDIDATE_RESERVED_CALLS_V1
            || self.reserved_work_lamports != self.work_schedule.reserved_work_lamports()?
        {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        self.first_call_ordinal
            .checked_add(self.reserved_calls)
            .ok_or(DirectMarketErrorV1::Arithmetic)?;
        Ok(())
    }
}

/// Product-authenticated private allocation token consumed only by foundation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedDirectCandidateLivenessV1 {
    binding: DirectCandidateLivenessBindingV1,
}

impl AuthenticatedDirectCandidateLivenessV1 {
    /// Exact compact binding persisted by `0xb1/1`.
    pub const fn binding(self) -> DirectCandidateLivenessBindingV1 {
        self.binding
    }
}

/// Default-deny Product/global-bundle authentication seam.
pub trait AuthenticatedDirectCandidateLivenessAuthorityV1 {
    /// Authenticate the exact global bundle, allocation range, and Product
    /// funding successor before minting Direct's private token.
    fn authenticate_candidate_allocation(
        &self,
        _binding: DirectCandidateLivenessBindingV1,
        _market_instance_id: [u8; 32],
        _generation: u64,
        _direct_root_account: [u8; 32],
        _action_replay_account: [u8; 32],
        _family_admission_sequence: u32,
        _realm_id: [u8; 32],
        _neutral_lamport_sink: [u8; 32],
        _candidate_lifecycle_policy_id: [u8; 32],
        _candidate_liveness_policy_id: [u8; 32],
    ) -> Result<(), DirectMarketErrorV1> {
        Err(DirectMarketErrorV1::UnauthenticatedAuthority)
    }
}

/// Explicit refusing Product allocation authority.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoDirectCandidateLivenessAuthorityV1;

impl AuthenticatedDirectCandidateLivenessAuthorityV1
    for NoDirectCandidateLivenessAuthorityV1
{
}

/// Mint the compact Direct token only after the streamed seven-row transcript
/// and Product's private allocation authority agree exactly.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_direct_candidate_liveness_v1<
    A: AuthenticatedDirectCandidateLivenessAuthorityV1 + ?Sized,
    S: DirectGlobalLivenessBundleStreamV1 + ?Sized,
    B: DirectHashBackendV1,
>(
    authority: &A,
    stream: &S,
    binding: DirectCandidateLivenessBindingV1,
    market_instance_id: [u8; 32],
    generation: u64,
    direct_root_account: [u8; 32],
    action_replay_account: [u8; 32],
    family_admission_sequence: u32,
    realm_id: [u8; 32],
    neutral_lamport_sink: [u8; 32],
    candidate_lifecycle_policy_id: [u8; 32],
    candidate_liveness_policy_id: [u8; 32],
    backend: &B,
) -> Result<AuthenticatedDirectCandidateLivenessV1, DirectMarketErrorV1> {
    binding.validate()?;
    let expected_schedule_id = binding.work_schedule.semantic_id(
        market_instance_id,
        generation,
        direct_root_account,
        family_admission_sequence,
        candidate_lifecycle_policy_id,
        candidate_liveness_policy_id,
        backend,
    )?;
    let expected_bundle_commitment = direct_global_liveness_bundle_commitment_v1(
        stream,
        binding.policy_account,
        candidate_liveness_policy_id,
        binding.policy_data_id,
        binding.global_lifecycle_id,
        binding.global_bundle_binding_id,
        binding.global_capitalization_receipt_id,
        backend,
    )?;
    let candidate = stream.compartment(RuntimeCompartmentKindV1::Candidate)?;
    if binding.work_schedule_id != expected_schedule_id
        || binding.global_bundle_commitment_id != expected_bundle_commitment
        || candidate.account_id != binding.candidate_account
        || candidate.account_data_id != binding.candidate_data_id
        || candidate.semantic_owner != binding.candidate_semantic_owner
        || candidate.quote_schedule_id != binding.candidate_quote_schedule_id
        || candidate.receipt_program_id != binding.candidate_receipt_program_id
        || candidate.generation != binding.candidate_generation
    {
        return Err(DirectMarketErrorV1::MismatchedBinding);
    }
    authority.authenticate_candidate_allocation(
        binding,
        market_instance_id,
        generation,
        direct_root_account,
        action_replay_account,
        family_admission_sequence,
        realm_id,
        neutral_lamport_sink,
        candidate_lifecycle_policy_id,
        candidate_liveness_policy_id,
    )?;
    Ok(AuthenticatedDirectCandidateLivenessV1 { binding })
}

const fn runtime_compartment_byte_v1(kind: RuntimeCompartmentKindV1) -> u8 {
    match kind {
        RuntimeCompartmentKindV1::Source => 0,
        RuntimeCompartmentKindV1::Candidate => 1,
        RuntimeCompartmentKindV1::Clearing => 2,
        RuntimeCompartmentKindV1::Settlement => 3,
        RuntimeCompartmentKindV1::Resolution => 4,
        RuntimeCompartmentKindV1::Retirement => 5,
        RuntimeCompartmentKindV1::Recovery => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[derive(Clone, Copy, Debug)]
    struct Sha;

    impl DirectHashBackendV1 for Sha {
        fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
            let mut hash = Sha256::new();
            for part in parts {
                hash.update(part);
            }
            hash.finalize().into()
        }
    }

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn schedule() -> DirectCandidateWorkScheduleV1 {
        DirectCandidateWorkScheduleV1 {
            freeze_book_lamports: 11,
            begin_verification_lamports: 13,
            verify_candidate_lamports: 17,
            finalize_selection_lamports: 19,
            economic_terminal_lamports: 23,
            retire_terminal_lamports: 29,
            retained_candidate_bond_lamports: 31,
        }
    }

    #[derive(Clone, Copy, Debug)]
    struct Stream {
        rows: [DirectGlobalLivenessCompartmentV1; RUNTIME_COMPARTMENT_COUNT_V1],
    }

    impl DirectGlobalLivenessBundleStreamV1 for Stream {
        fn compartment(
            &self,
            kind: RuntimeCompartmentKindV1,
        ) -> Result<DirectGlobalLivenessCompartmentV1, DirectMarketErrorV1> {
            Ok(self.rows[kind.index()])
        }
    }

    fn stream() -> Stream {
        let first = DirectGlobalLivenessCompartmentV1 {
            kind: RuntimeCompartmentKindV1::Source,
            account_id: id(20),
            account_data_id: id(30),
            capitalization_receipt_id: id(40),
            semantic_owner: id(50),
            quote_schedule_id: id(60),
            receipt_program_id: id(70),
            generation: 1,
        };
        let mut rows = [first; RUNTIME_COMPARTMENT_COUNT_V1];
        let mut index = 0usize;
        while index < RUNTIME_COMPARTMENT_COUNT_V1 {
            rows[index] = DirectGlobalLivenessCompartmentV1 {
                kind: RUNTIME_COMPARTMENT_ORDER_V1[index],
                account_id: id(u8::try_from(20 + index).unwrap()),
                account_data_id: id(u8::try_from(30 + index).unwrap()),
                capitalization_receipt_id: id(u8::try_from(40 + index).unwrap()),
                semantic_owner: id(u8::try_from(50 + index).unwrap()),
                quote_schedule_id: id(u8::try_from(60 + index).unwrap()),
                receipt_program_id: id(70),
                generation: 1,
            };
            index += 1;
        }
        Stream { rows }
    }

    #[derive(Clone, Copy, Debug)]
    struct Allow;

    impl AuthenticatedDirectCandidateLivenessAuthorityV1 for Allow {
        fn authenticate_candidate_allocation(
            &self,
            _binding: DirectCandidateLivenessBindingV1,
            _market_instance_id: [u8; 32],
            _generation: u64,
            _direct_root_account: [u8; 32],
            _action_replay_account: [u8; 32],
            _family_admission_sequence: u32,
            _realm_id: [u8; 32],
            _neutral_lamport_sink: [u8; 32],
            _candidate_lifecycle_policy_id: [u8; 32],
            _candidate_liveness_policy_id: [u8; 32],
        ) -> Result<(), DirectMarketErrorV1> {
            Ok(())
        }
    }

    fn binding(stream: &Stream) -> DirectCandidateLivenessBindingV1 {
        let work_schedule = schedule();
        let work_schedule_id = work_schedule
            .semantic_id(id(1), 1, id(2), 0, id(3), id(4), &Sha)
            .unwrap();
        let global_bundle_commitment_id = direct_global_liveness_bundle_commitment_v1(
            stream,
            id(5),
            id(4),
            id(6),
            id(7),
            id(8),
            id(9),
            &Sha,
        )
        .unwrap();
        let candidate = stream
            .compartment(RuntimeCompartmentKindV1::Candidate)
            .unwrap();
        DirectCandidateLivenessBindingV1 {
            policy_account: id(5),
            policy_data_id: id(6),
            global_lifecycle_id: id(7),
            global_bundle_binding_id: id(8),
            global_capitalization_receipt_id: id(9),
            global_bundle_commitment_id,
            candidate_account: candidate.account_id,
            candidate_data_id: candidate.account_data_id,
            candidate_semantic_owner: candidate.semantic_owner,
            candidate_quote_schedule_id: candidate.quote_schedule_id,
            candidate_receipt_program_id: candidate.receipt_program_id,
            candidate_generation: candidate.generation,
            first_call_ordinal: 1,
            reserved_calls: DIRECT_CANDIDATE_RESERVED_CALLS_V1,
            reserved_work_lamports: work_schedule.reserved_work_lamports().unwrap(),
            allocation_receipt_id: id(10),
            work_schedule,
            work_schedule_id,
        }
    }

    #[test]
    fn exact_streamed_bundle_and_private_authority_mint_allocation() {
        let stream = stream();
        let value = binding(&stream);
        let authenticated = authenticate_direct_candidate_liveness_v1(
            &Allow,
            &stream,
            value,
            id(1),
            1,
            id(2),
            id(11),
            0,
            id(12),
            id(13),
            id(3),
            id(4),
            &Sha,
        )
        .unwrap();
        assert_eq!(authenticated.binding(), value);
    }

    #[test]
    fn missing_duplicate_or_mutated_global_row_cannot_mint_allocation() {
        let canonical = stream();
        let value = binding(&canonical);
        let mut duplicate = canonical;
        duplicate.rows[2].account_id = duplicate.rows[1].account_id;
        assert_eq!(
            authenticate_direct_candidate_liveness_v1(
                &Allow, &duplicate, value, id(1), 1, id(2), id(11), 0, id(12), id(13),
                id(3), id(4), &Sha,
            ),
            Err(DirectMarketErrorV1::IdentityAlias),
        );

        let mut mutated = canonical;
        mutated.rows[1].account_data_id = id(99);
        assert_eq!(
            authenticate_direct_candidate_liveness_v1(
                &Allow, &mutated, value, id(1), 1, id(2), id(11), 0, id(12), id(13),
                id(3), id(4), &Sha,
            ),
            Err(DirectMarketErrorV1::MismatchedBinding),
        );
    }

    #[test]
    fn caller_projection_and_underfunded_schedule_remain_refused() {
        let stream = stream();
        let value = binding(&stream);
        assert_eq!(
            authenticate_direct_candidate_liveness_v1(
                &NoDirectCandidateLivenessAuthorityV1,
                &stream,
                value,
                id(1),
                1,
                id(2),
                id(11),
                0,
                id(12),
                id(13),
                id(3),
                id(4),
                &Sha,
            ),
            Err(DirectMarketErrorV1::UnauthenticatedAuthority),
        );

        let mut underfunded = value;
        underfunded.reserved_work_lamports -= 1;
        assert_eq!(
            underfunded.validate(),
            Err(DirectMarketErrorV1::InvalidCount),
        );
    }
}
