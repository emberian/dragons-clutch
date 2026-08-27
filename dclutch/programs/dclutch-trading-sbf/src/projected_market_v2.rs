//! Compact family-neutral projected-Market execution coordinates.
//!
//! The outer instruction carries only one contiguous family request. It never
//! carries a caller-authored Core request, the already-persisted ProjectFound
//! receipt, a projected Custody request, or an intermediate child receipt.
//! The authenticated program graph derives both prefix child requests; the
//! projected values are cross-checked against the authenticated Custody state
//! and retained only for the duration of the instruction.

use dclutch_custody_contract::{
    ProjectedCustodyOperationV1, ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1,
    ProjectedCustodyStateV1,
};
use dclutch_market_core_codec::{
    Action, Identity, ProjectFoundReceiptV1, Request, SeriesCoreFoundAckV2, SeriesCoreRequestV1,
};
use solana_program::hash::hash;

/// Compact projected-execution instruction magic.
pub const PROJECTED_MARKET_EXECUTION_MAGIC_V2: [u8; 8] = *b"DCLTPX02";
/// Exact compact preamble width.
pub const PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V2: usize = 16;
/// Exact semantic family-header width before its word-aligned witness.
pub const PROJECTED_MARKET_FAMILY_HEADER_BYTES_V2: usize = 128;
/// Exact bytes in one projected witness word.
pub const PROJECTED_MARKET_WITNESS_WORD_BYTES_V2: usize = 32;
/// Largest admitted affine child span for the initial physical profile.
pub const PROJECTED_MARKET_MAX_AFFINE_COUNT_V2: u8 = 16;
/// Fixed bytes before the witness words.
pub const PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2: usize =
    PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V2 + PROJECTED_MARKET_FAMILY_HEADER_BYTES_V2;

const VERSION_V2: u16 = 2;
const WITNESS_WORDS_OFFSET: usize = 10;
const AFFINE_COUNT_OFFSET: usize = 11;
const RESERVED_OFFSET: usize = 12;

/// Stable refusal from compact projected execution or deterministic replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectedMarketExecutionErrorV2 {
    /// The byte slice did not have its exact count-derived width.
    InvalidLength,
    /// Magic or version selected another schema.
    InvalidHeader,
    /// Reserved bytes or count fields were not canonical.
    NonCanonical,
    /// The projected state was not the empty pre-Fond Hoard authority.
    ProjectedState,
    /// A persisted ProjectFound commitment could not be reconstructed exactly.
    Projection,
    /// The next projected-Custody Lock request could not be constructed.
    LockRequest,
    /// Core return provenance or the Found-only funding attestation refused.
    FoundAcknowledgement,
}

/// Borrowed exact compact instruction partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedMarketExecutionV2<'a> {
    witness_words: u8,
    affine_count: u8,
    family_request: &'a [u8],
}

/// Core-promoted affine span admitted for the live-Market continuation.
///
/// The compact preamble count is only a routing hint until this value exists.
/// It can be constructed only from the current Core program's exact raw
/// `SeriesCoreFoundAckV2`, joined to the derived Core request and independently
/// authenticated ordered FundingState-list identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedFoundSpanV2 {
    funding_count: u8,
    funding_list_id: [u8; 32],
    acknowledgement_digest: [u8; 32],
}

impl AuthenticatedFoundSpanV2 {
    /// Core-authenticated nonzero FundingState count.
    pub const fn funding_count(self) -> u8 {
        self.funding_count
    }

    /// Core-authenticated exact ordered FundingState-list identity.
    pub const fn funding_list_id(self) -> [u8; 32] {
        self.funding_list_id
    }

    /// SHA-256 of the exact raw Core return bytes retained by the outer.
    pub const fn acknowledgement_digest(self) -> [u8; 32] {
        self.acknowledgement_digest
    }
}

impl<'a> ProjectedMarketExecutionV2<'a> {
    /// Hostile-decode the sole compact wire.
    pub fn decode(input: &'a [u8]) -> Result<Self, ProjectedMarketExecutionErrorV2> {
        if input.len() < PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2
            || input.get(..8) != Some(PROJECTED_MARKET_EXECUTION_MAGIC_V2.as_slice())
            || read_u16(input, 8)? != VERSION_V2
        {
            return Err(ProjectedMarketExecutionErrorV2::InvalidHeader);
        }
        if input
            .get(RESERVED_OFFSET..PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V2)
            .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(ProjectedMarketExecutionErrorV2::NonCanonical);
        }
        let witness_words = read_u8(input, WITNESS_WORDS_OFFSET)?;
        let affine_count = read_u8(input, AFFINE_COUNT_OFFSET)?;
        if affine_count == 0 || affine_count > PROJECTED_MARKET_MAX_AFFINE_COUNT_V2 {
            return Err(ProjectedMarketExecutionErrorV2::NonCanonical);
        }
        let expected = PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2
            .checked_add(
                usize::from(witness_words)
                    .checked_mul(PROJECTED_MARKET_WITNESS_WORD_BYTES_V2)
                    .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)?,
            )
            .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)?;
        if input.len() != expected {
            return Err(ProjectedMarketExecutionErrorV2::InvalidLength);
        }
        let family_start = PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V2;
        Ok(Self {
            witness_words,
            affine_count,
            family_request: input
                .get(family_start..)
                .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)?,
        })
    }

    /// Exact witness-word count declared by the compact preamble.
    pub const fn witness_words(self) -> u8 {
        self.witness_words
    }

    /// Bounded pre-child affine-count hint.
    pub const fn affine_count(self) -> u8 {
        self.affine_count
    }

    /// Contiguous semantic family header and witness, with no duplicate proof.
    pub const fn family_request(self) -> &'a [u8] {
        self.family_request
    }

    /// Exact witness suffix following the fixed family header.
    pub fn witness(self) -> &'a [u8] {
        self.family_request
            .get(PROJECTED_MARKET_FAMILY_HEADER_BYTES_V2..)
            .unwrap_or(&[])
    }
}

/// Encode the compact projected instruction into exact caller-owned storage.
pub fn encode_projected_market_execution_v2(
    output: &mut [u8],
    family_header: &[u8; PROJECTED_MARKET_FAMILY_HEADER_BYTES_V2],
    witness: &[u8],
    affine_count: u8,
) -> Result<(), ProjectedMarketExecutionErrorV2> {
    if !witness
        .len()
        .is_multiple_of(PROJECTED_MARKET_WITNESS_WORD_BYTES_V2)
        || affine_count == 0
        || affine_count > PROJECTED_MARKET_MAX_AFFINE_COUNT_V2
    {
        return Err(ProjectedMarketExecutionErrorV2::NonCanonical);
    }
    let witness_words = witness.len() / PROJECTED_MARKET_WITNESS_WORD_BYTES_V2;
    let witness_words =
        u8::try_from(witness_words).map_err(|_| ProjectedMarketExecutionErrorV2::InvalidLength)?;
    let expected = PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2
        .checked_add(witness.len())
        .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)?;
    if output.len() != expected {
        return Err(ProjectedMarketExecutionErrorV2::InvalidLength);
    }
    output.fill(0);
    put(output, 0, &PROJECTED_MARKET_EXECUTION_MAGIC_V2)?;
    put(output, 8, &VERSION_V2.to_le_bytes())?;
    put_u8(output, WITNESS_WORDS_OFFSET, witness_words)?;
    put_u8(output, AFFINE_COUNT_OFFSET, affine_count)?;
    let family_start = PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V2;
    put(output, family_start, family_header)?;
    put(
        output,
        family_start
            .checked_add(PROJECTED_MARKET_FAMILY_HEADER_BYTES_V2)
            .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)?,
        witness,
    )?;
    Ok(())
}

/// Reconstruct and authenticate the exact earlier Core ProjectFound receipt.
///
/// The persisted projected state is the sole owner of these facts.  No caller
/// supplies projection bytes, and the reconstructed receipt must hash to the
/// commitment already stored by Custody.
pub fn reconstruct_project_found_v1(
    state: ProjectedCustodyStateV1,
) -> Result<ProjectFoundReceiptV1, ProjectedMarketExecutionErrorV2> {
    require_open_projected_state(state)?;
    let market = Identity::new(state.request.market)
        .map_err(|_| ProjectedMarketExecutionErrorV2::Projection)?;
    let found = Request::administrative(Action::Found, state.request.generation, market);
    let found_bytes = found
        .encode()
        .map_err(|_| ProjectedMarketExecutionErrorV2::Projection)?;
    let receipt = ProjectFoundReceiptV1::new(
        market,
        state.request.generation,
        identity(state.request.realm)?,
        identity(state.request.mint)?,
        identity(state.request.token_program)?,
        identity(state.request.collateral_release)?,
        identity(state.request.product_record)?,
        identity(state.request.product)?,
        identity(state.request.source)?,
        identity(state.request.release_set)?,
        identity(state.request.rent_program)?,
        hash(&found_bytes).to_bytes(),
    )
    .map_err(|_| ProjectedMarketExecutionErrorV2::Projection)?;
    let receipt_bytes = receipt
        .encode()
        .map_err(|_| ProjectedMarketExecutionErrorV2::Projection)?;
    if hash(&receipt_bytes).to_bytes() != state.request.projection_receipt_digest {
        return Err(ProjectedMarketExecutionErrorV2::Projection);
    }
    Ok(receipt)
}

/// Derive the unique next Lock-and-close-source request from authenticated
/// HoardOpen state and the admitted positive principal.
pub fn reconstruct_projected_lock_v1(
    state: ProjectedCustodyStateV1,
    amount: u64,
) -> Result<ProjectedCustodyRequestV1, ProjectedMarketExecutionErrorV2> {
    require_open_projected_state(state)?;
    let resulting_revision = state
        .next_revision
        .checked_add(1)
        .ok_or(ProjectedMarketExecutionErrorV2::LockRequest)?;
    let request = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
        expected_revision: state.next_revision,
        resulting_revision,
        amount,
        ..state.request
    };
    request
        .validate()
        .map_err(|_| ProjectedMarketExecutionErrorV2::LockRequest)?;
    Ok(request)
}

/// Promote the bounded preamble hint through the sole current-Core authority.
///
/// `funding_list_id` and `observed_post_resource_digest` must be derived from
/// the authenticated ordered FundingState accounts and live Market/permit
/// poststate, respectively. The raw return producer is checked separately from
/// the identities echoed by the typed receipt so substituted return data from
/// another executable program cannot promote the continuation span.
#[allow(clippy::too_many_arguments)]
pub fn authenticate_series_found_span_v2(
    execution: ProjectedMarketExecutionV2<'_>,
    return_data_producer: [u8; 32],
    raw_acknowledgement: &[u8],
    core_request: SeriesCoreRequestV1,
    expected_core_program: [u8; 32],
    expected_permit: [u8; 32],
    core_request_digest: [u8; 32],
    funding_list_id: [u8; 32],
    observed_post_resource_digest: [u8; 32],
) -> Result<AuthenticatedFoundSpanV2, ProjectedMarketExecutionErrorV2> {
    if return_data_producer != expected_core_program {
        return Err(ProjectedMarketExecutionErrorV2::FoundAcknowledgement);
    }
    let acknowledgement = SeriesCoreFoundAckV2::decode(raw_acknowledgement)
        .map_err(|_| ProjectedMarketExecutionErrorV2::FoundAcknowledgement)?;
    let funding_count = execution.affine_count();
    if acknowledgement.funding_count() != funding_count {
        return Err(ProjectedMarketExecutionErrorV2::FoundAcknowledgement);
    }
    acknowledgement
        .validate_for(
            core_request,
            identity_ack(expected_core_program)?,
            identity_ack(expected_permit)?,
            identity_ack(core_request_digest)?,
            funding_count,
            identity_ack(funding_list_id)?,
            identity_ack(observed_post_resource_digest)?,
        )
        .map_err(|_| ProjectedMarketExecutionErrorV2::FoundAcknowledgement)?;
    Ok(AuthenticatedFoundSpanV2 {
        funding_count,
        funding_list_id,
        acknowledgement_digest: hash(raw_acknowledgement).to_bytes(),
    })
}

fn require_open_projected_state(
    state: ProjectedCustodyStateV1,
) -> Result<(), ProjectedMarketExecutionErrorV2> {
    if state.phase != ProjectedCustodyPhaseV1::HoardOpen
        || state.locked_amount != 0
        || state.request.operation != ProjectedCustodyOperationV1::OpenHoard
        || state.request.amount != 0
        || state.next_revision == 0
    {
        return Err(ProjectedMarketExecutionErrorV2::ProjectedState);
    }
    Ok(())
}

fn identity(value: [u8; 32]) -> Result<Identity, ProjectedMarketExecutionErrorV2> {
    Identity::new(value).map_err(|_| ProjectedMarketExecutionErrorV2::Projection)
}

fn identity_ack(value: [u8; 32]) -> Result<Identity, ProjectedMarketExecutionErrorV2> {
    Identity::new(value).map_err(|_| ProjectedMarketExecutionErrorV2::FoundAcknowledgement)
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8, ProjectedMarketExecutionErrorV2> {
    input
        .get(offset)
        .copied()
        .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, ProjectedMarketExecutionErrorV2> {
    Ok(u16::from_le_bytes(
        slice(input, offset, 2)?
            .try_into()
            .map_err(|_| ProjectedMarketExecutionErrorV2::InvalidLength)?,
    ))
}

fn slice(
    input: &[u8],
    offset: usize,
    width: usize,
) -> Result<&[u8], ProjectedMarketExecutionErrorV2> {
    input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)?,
        )
        .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)
}

fn put(
    output: &mut [u8],
    offset: usize,
    value: &[u8],
) -> Result<(), ProjectedMarketExecutionErrorV2> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)?;
    let destination = output
        .get_mut(offset..end)
        .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)?;
    destination.copy_from_slice(value);
    Ok(())
}

fn put_u8(
    output: &mut [u8],
    offset: usize,
    value: u8,
) -> Result<(), ProjectedMarketExecutionErrorV2> {
    *output
        .get_mut(offset)
        .ok_or(ProjectedMarketExecutionErrorV2::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use alloc::vec;
    use dclutch_custody_contract::{CompartmentV1, ProjectedCallerRoleV1, ProjectedCustodyPhaseV1};

    use super::*;

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn open_state() -> ProjectedCustodyStateV1 {
        let mut request = ProjectedCustodyRequestV1 {
            operation: ProjectedCustodyOperationV1::OpenHoard,
            caller_role: ProjectedCallerRoleV1::TradingCapability,
            market: id(1),
            generation: 7,
            realm: id(2),
            product_record: id(3),
            product: id(4),
            source: id(5),
            release_set: id(6),
            projection_receipt_digest: id(7),
            parent_capability_root: id(8),
            context_digest: id(9),
            caller_program: id(10),
            payer: id(11),
            core_program: id(12),
            rent_program: id(13),
            refund_owner: id(14),
            rent_credit: id(15),
            hoard_vault: id(16),
            funding_source_vault: id(17),
            funding_source_context: id(18),
            funding_source_compartment: CompartmentV1::SeriesEscrow,
            mint: id(19),
            token_program: id(20),
            collateral_release: id(21),
            expiry_slot: 100,
            expected_revision: 1,
            resulting_revision: 2,
            amount: 0,
            state_rent_lamports: 1_000,
            vault_rent_lamports: 2_000,
            funding_source_replay_revision: 3,
            funding_source_state_rent_lamports: 3_000,
            funding_source_vault_rent_lamports: 4_000,
        };
        let provisional = ProjectedCustodyStateV1 {
            phase: ProjectedCustodyPhaseV1::HoardOpen,
            request,
            next_revision: 2,
            locked_amount: 0,
            last_request_digest: id(22),
            bump: 254,
        };
        let market = Identity::new(request.market).expect("market");
        let found = Request::administrative(Action::Found, request.generation, market)
            .encode()
            .expect("Found bytes");
        let receipt = ProjectFoundReceiptV1::new(
            market,
            request.generation,
            identity(request.realm).expect("realm"),
            identity(request.mint).expect("mint"),
            identity(request.token_program).expect("token program"),
            identity(request.collateral_release).expect("collateral release"),
            identity(request.product_record).expect("Product record"),
            identity(request.product).expect("Product"),
            identity(request.source).expect("Source"),
            identity(request.release_set).expect("release set"),
            identity(request.rent_program).expect("Rent"),
            hash(&found).to_bytes(),
        )
        .expect("receipt");
        request.projection_receipt_digest = hash(&receipt.encode().expect("encode")).to_bytes();
        ProjectedCustodyStateV1 {
            request,
            ..provisional
        }
    }

    fn core_request() -> SeriesCoreRequestV1 {
        SeriesCoreRequestV1::occurrence(
            dclutch_market_core_codec::SeriesCoreActionV1::Consume,
            identity_ack(id(30)).expect("release"),
            identity_ack(id(31)).expect("Template"),
            identity_ack(id(32)).expect("Ticket"),
            identity_ack(id(33)).expect("Market"),
            identity_ack(id(34)).expect("Product"),
            identity_ack(id(35)).expect("Source"),
            identity_ack(id(36)).expect("founder"),
            identity_ack(id(37)).expect("RentCredit"),
            38,
            39,
            40,
            41,
            42,
            43,
            44,
        )
        .expect("Consume request")
    }

    fn execution(affine_count: u8) -> alloc::vec::Vec<u8> {
        let header = [0x44; PROJECTED_MARKET_FAMILY_HEADER_BYTES_V2];
        let mut output = vec![0_u8; PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2];
        encode_projected_market_execution_v2(&mut output, &header, &[], affine_count)
            .expect("compact execution");
        output
    }

    #[test]
    fn compact_wire_keeps_one_contiguous_family_request() {
        let header = [0x44; PROJECTED_MARKET_FAMILY_HEADER_BYTES_V2];
        let witness = [0x55; 9 * PROJECTED_MARKET_WITNESS_WORD_BYTES_V2];
        let mut output = vec![0_u8; PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2 + witness.len()];
        encode_projected_market_execution_v2(&mut output, &header, &witness, 5).expect("encode");
        let decoded = ProjectedMarketExecutionV2::decode(&output).expect("decode");
        assert_eq!(output.len(), 432);
        assert_eq!(decoded.witness_words(), 9);
        assert_eq!(decoded.affine_count(), 5);
        assert_eq!(
            decoded.family_request(),
            [&header[..], &witness[..]].concat()
        );
        assert_eq!(decoded.witness(), witness);
    }

    #[test]
    fn compact_wire_refuses_reserved_width_and_affine_substitution() {
        let header = [0x44; PROJECTED_MARKET_FAMILY_HEADER_BYTES_V2];
        let witness = [0x55; PROJECTED_MARKET_WITNESS_WORD_BYTES_V2];
        let mut output = vec![0_u8; PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V2 + witness.len()];
        encode_projected_market_execution_v2(&mut output, &header, &witness, 1).expect("encode");

        let mut changed = output.clone();
        *changed.get_mut(RESERVED_OFFSET).expect("reserved byte") = 1;
        assert_eq!(
            ProjectedMarketExecutionV2::decode(&changed),
            Err(ProjectedMarketExecutionErrorV2::NonCanonical)
        );
        changed = output.clone();
        *changed.get_mut(AFFINE_COUNT_OFFSET).expect("affine count") = 0;
        assert_eq!(
            ProjectedMarketExecutionV2::decode(&changed),
            Err(ProjectedMarketExecutionErrorV2::NonCanonical)
        );
        changed = output.clone();
        *changed
            .get_mut(WITNESS_WORDS_OFFSET)
            .expect("witness words") = 2;
        assert_eq!(
            ProjectedMarketExecutionV2::decode(&changed),
            Err(ProjectedMarketExecutionErrorV2::InvalidLength)
        );
        assert_eq!(
            ProjectedMarketExecutionV2::decode(
                output
                    .get(..output.len() - 1)
                    .expect("one-byte-short fixture"),
            ),
            Err(ProjectedMarketExecutionErrorV2::InvalidLength)
        );
    }

    #[test]
    fn persisted_state_reconstructs_projection_and_unique_next_lock() {
        let state = open_state();
        let receipt = reconstruct_project_found_v1(state).expect("exact projection");
        assert_eq!(receipt.market.to_bytes(), state.request.market);
        assert_eq!(receipt.generation, state.request.generation);

        let lock = reconstruct_projected_lock_v1(state, 88).expect("next lock");
        assert_eq!(
            lock.operation,
            ProjectedCustodyOperationV1::LockHoardAndCloseSource
        );
        assert_eq!(lock.expected_revision, 2);
        assert_eq!(lock.resulting_revision, 3);
        assert_eq!(lock.amount, 88);
        assert_eq!(
            lock.projection_receipt_digest,
            state.request.projection_receipt_digest
        );
    }

    #[test]
    fn reconstruction_refuses_projection_or_phase_substitution() {
        let state = open_state();
        let mut changed = state;
        changed.request.projection_receipt_digest = id(31);
        assert_eq!(
            reconstruct_project_found_v1(changed),
            Err(ProjectedMarketExecutionErrorV2::Projection)
        );

        changed = state;
        changed.phase = ProjectedCustodyPhaseV1::Initialized;
        assert_eq!(
            reconstruct_projected_lock_v1(changed, 88),
            Err(ProjectedMarketExecutionErrorV2::ProjectedState)
        );
        assert_eq!(
            reconstruct_projected_lock_v1(state, 0),
            Err(ProjectedMarketExecutionErrorV2::LockRequest)
        );
    }

    #[test]
    fn only_current_core_ack_promotes_the_affine_hint() {
        let request = core_request();
        let core_program = id(51);
        let permit = id(52);
        let request_digest = id(53);
        let funding_list = id(54);
        let post_resource = id(55);
        let raw_acknowledgement = SeriesCoreFoundAckV2::new(
            request,
            identity_ack(core_program).expect("Core"),
            identity_ack(permit).expect("permit"),
            identity_ack(request_digest).expect("request digest"),
            3,
            identity_ack(funding_list).expect("funding list"),
            identity_ack(post_resource).expect("post resource"),
        )
        .expect("Found acknowledgement")
        .encode()
        .expect("acknowledgement bytes");
        let wire = execution(3);
        let decoded = ProjectedMarketExecutionV2::decode(&wire).expect("compact wire");
        let promoted = authenticate_series_found_span_v2(
            decoded,
            core_program,
            &raw_acknowledgement,
            request,
            core_program,
            permit,
            request_digest,
            funding_list,
            post_resource,
        )
        .expect("Core-promoted span");
        assert_eq!(promoted.funding_count(), 3);
        assert_eq!(promoted.funding_list_id(), funding_list);
        assert_eq!(
            promoted.acknowledgement_digest(),
            hash(&raw_acknowledgement).to_bytes()
        );
    }

    #[test]
    fn producer_hint_and_funding_substitution_cannot_promote() {
        let request = core_request();
        let core_program = id(51);
        let permit = id(52);
        let request_digest = id(53);
        let funding_list = id(54);
        let post_resource = id(55);
        let raw_acknowledgement = SeriesCoreFoundAckV2::new(
            request,
            identity_ack(core_program).expect("Core"),
            identity_ack(permit).expect("permit"),
            identity_ack(request_digest).expect("request digest"),
            3,
            identity_ack(funding_list).expect("funding list"),
            identity_ack(post_resource).expect("post resource"),
        )
        .expect("Found acknowledgement")
        .encode()
        .expect("acknowledgement bytes");
        let wire = execution(3);
        let decoded = ProjectedMarketExecutionV2::decode(&wire).expect("compact wire");
        let authenticate = |execution, producer, list| {
            authenticate_series_found_span_v2(
                execution,
                producer,
                &raw_acknowledgement,
                request,
                core_program,
                permit,
                request_digest,
                list,
                post_resource,
            )
        };
        assert_eq!(
            authenticate(decoded, id(56), funding_list),
            Err(ProjectedMarketExecutionErrorV2::FoundAcknowledgement)
        );
        assert_eq!(
            authenticate(decoded, core_program, id(57)),
            Err(ProjectedMarketExecutionErrorV2::FoundAcknowledgement)
        );
        let changed_wire = execution(2);
        let changed = ProjectedMarketExecutionV2::decode(&changed_wire).expect("changed hint");
        assert_eq!(
            authenticate(changed, core_program, funding_list),
            Err(ProjectedMarketExecutionErrorV2::FoundAcknowledgement)
        );
    }
}
