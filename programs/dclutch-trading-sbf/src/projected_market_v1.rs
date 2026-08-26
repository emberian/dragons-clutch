//! Compact family-neutral projected-Market execution coordinates.
//!
//! The outer instruction carries one fixed Core request and one contiguous
//! family request.  It never carries the already-persisted ProjectFound
//! receipt, projected Custody request, or an intermediate child receipt.
//! Those values are reconstructed from the authenticated projected-Custody
//! state and retained only for the duration of the instruction.

use dclutch_custody_contract::{
    ProjectedCustodyOperationV1, ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1,
    ProjectedCustodyStateV1,
};
use dclutch_market_core_codec::{
    Action, Identity, ProjectFoundReceiptV1, Request, SERIES_CORE_REQUEST_BYTES_V1,
};
use solana_program::hash::hash;

/// Compact projected-execution instruction magic.
pub const PROJECTED_MARKET_EXECUTION_MAGIC_V1: [u8; 8] = *b"DCLTPX01";
/// Exact compact preamble width.
pub const PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V1: usize = 16;
/// Exact semantic family-header width before its word-aligned witness.
pub const PROJECTED_MARKET_FAMILY_HEADER_BYTES_V1: usize = 128;
/// Exact bytes in one projected witness word.
pub const PROJECTED_MARKET_WITNESS_WORD_BYTES_V1: usize = 32;
/// Largest admitted affine child span for the initial physical profile.
pub const PROJECTED_MARKET_MAX_AFFINE_COUNT_V1: u8 = 16;
/// Fixed bytes before the witness words.
pub const PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V1: usize =
    PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V1
        + SERIES_CORE_REQUEST_BYTES_V1
        + PROJECTED_MARKET_FAMILY_HEADER_BYTES_V1;

const VERSION_V1: u16 = 1;
const WITNESS_WORDS_OFFSET: usize = 10;
const AFFINE_COUNT_OFFSET: usize = 11;
const RESERVED_OFFSET: usize = 12;

/// Stable refusal from compact projected execution or deterministic replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectedMarketExecutionErrorV1 {
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
}

/// Borrowed exact compact instruction partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectedMarketExecutionV1<'a> {
    witness_words: u8,
    affine_count: u8,
    core_request: &'a [u8],
    family_request: &'a [u8],
}

impl<'a> ProjectedMarketExecutionV1<'a> {
    /// Hostile-decode the sole compact wire.
    pub fn decode(input: &'a [u8]) -> Result<Self, ProjectedMarketExecutionErrorV1> {
        if input.len() < PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V1
            || input.get(..8) != Some(PROJECTED_MARKET_EXECUTION_MAGIC_V1.as_slice())
            || read_u16(input, 8)? != VERSION_V1
        {
            return Err(ProjectedMarketExecutionErrorV1::InvalidHeader);
        }
        if input
            .get(RESERVED_OFFSET..PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V1)
            .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(ProjectedMarketExecutionErrorV1::NonCanonical);
        }
        let witness_words = read_u8(input, WITNESS_WORDS_OFFSET)?;
        let affine_count = read_u8(input, AFFINE_COUNT_OFFSET)?;
        if affine_count == 0 || affine_count > PROJECTED_MARKET_MAX_AFFINE_COUNT_V1 {
            return Err(ProjectedMarketExecutionErrorV1::NonCanonical);
        }
        let expected = PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V1
            .checked_add(
                usize::from(witness_words)
                    .checked_mul(PROJECTED_MARKET_WITNESS_WORD_BYTES_V1)
                    .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?,
            )
            .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?;
        if input.len() != expected {
            return Err(ProjectedMarketExecutionErrorV1::InvalidLength);
        }
        let core_start = PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V1;
        let family_start = core_start
            .checked_add(SERIES_CORE_REQUEST_BYTES_V1)
            .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?;
        Ok(Self {
            witness_words,
            affine_count,
            core_request: slice(input, core_start, SERIES_CORE_REQUEST_BYTES_V1)?,
            family_request: input
                .get(family_start..)
                .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?,
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

    /// Exact fixed Core request bytes.
    pub const fn core_request(self) -> &'a [u8] {
        self.core_request
    }

    /// Contiguous semantic family header and witness, with no duplicate proof.
    pub const fn family_request(self) -> &'a [u8] {
        self.family_request
    }

    /// Exact witness suffix following the fixed family header.
    pub fn witness(self) -> &'a [u8] {
        self.family_request
            .get(PROJECTED_MARKET_FAMILY_HEADER_BYTES_V1..)
            .unwrap_or(&[])
    }
}

/// Encode the compact projected instruction into exact caller-owned storage.
pub fn encode_projected_market_execution_v1(
    output: &mut [u8],
    core_request: &[u8; SERIES_CORE_REQUEST_BYTES_V1],
    family_header: &[u8; PROJECTED_MARKET_FAMILY_HEADER_BYTES_V1],
    witness: &[u8],
    affine_count: u8,
) -> Result<(), ProjectedMarketExecutionErrorV1> {
    if !witness
        .len()
        .is_multiple_of(PROJECTED_MARKET_WITNESS_WORD_BYTES_V1)
        || affine_count == 0
        || affine_count > PROJECTED_MARKET_MAX_AFFINE_COUNT_V1
    {
        return Err(ProjectedMarketExecutionErrorV1::NonCanonical);
    }
    let witness_words = witness.len() / PROJECTED_MARKET_WITNESS_WORD_BYTES_V1;
    let witness_words =
        u8::try_from(witness_words).map_err(|_| ProjectedMarketExecutionErrorV1::InvalidLength)?;
    let expected = PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V1
        .checked_add(witness.len())
        .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?;
    if output.len() != expected {
        return Err(ProjectedMarketExecutionErrorV1::InvalidLength);
    }
    output.fill(0);
    put(output, 0, &PROJECTED_MARKET_EXECUTION_MAGIC_V1)?;
    put(output, 8, &VERSION_V1.to_le_bytes())?;
    put_u8(output, WITNESS_WORDS_OFFSET, witness_words)?;
    put_u8(output, AFFINE_COUNT_OFFSET, affine_count)?;
    let core_start = PROJECTED_MARKET_EXECUTION_PREAMBLE_BYTES_V1;
    put(output, core_start, core_request)?;
    let family_start = core_start
        .checked_add(SERIES_CORE_REQUEST_BYTES_V1)
        .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?;
    put(output, family_start, family_header)?;
    put(
        output,
        family_start
            .checked_add(PROJECTED_MARKET_FAMILY_HEADER_BYTES_V1)
            .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?,
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
) -> Result<ProjectFoundReceiptV1, ProjectedMarketExecutionErrorV1> {
    require_open_projected_state(state)?;
    let market = Identity::new(state.request.market)
        .map_err(|_| ProjectedMarketExecutionErrorV1::Projection)?;
    let found = Request::administrative(Action::Found, state.request.generation, market);
    let found_bytes = found
        .encode()
        .map_err(|_| ProjectedMarketExecutionErrorV1::Projection)?;
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
    .map_err(|_| ProjectedMarketExecutionErrorV1::Projection)?;
    let receipt_bytes = receipt
        .encode()
        .map_err(|_| ProjectedMarketExecutionErrorV1::Projection)?;
    if hash(&receipt_bytes).to_bytes() != state.request.projection_receipt_digest {
        return Err(ProjectedMarketExecutionErrorV1::Projection);
    }
    Ok(receipt)
}

/// Derive the unique next Lock-and-close-source request from authenticated
/// HoardOpen state and the admitted positive principal.
pub fn reconstruct_projected_lock_v1(
    state: ProjectedCustodyStateV1,
    amount: u64,
) -> Result<ProjectedCustodyRequestV1, ProjectedMarketExecutionErrorV1> {
    require_open_projected_state(state)?;
    let resulting_revision = state
        .next_revision
        .checked_add(1)
        .ok_or(ProjectedMarketExecutionErrorV1::LockRequest)?;
    let request = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
        expected_revision: state.next_revision,
        resulting_revision,
        amount,
        ..state.request
    };
    request
        .validate()
        .map_err(|_| ProjectedMarketExecutionErrorV1::LockRequest)?;
    Ok(request)
}

fn require_open_projected_state(
    state: ProjectedCustodyStateV1,
) -> Result<(), ProjectedMarketExecutionErrorV1> {
    if state.phase != ProjectedCustodyPhaseV1::HoardOpen
        || state.locked_amount != 0
        || state.request.operation != ProjectedCustodyOperationV1::OpenHoard
        || state.request.amount != 0
        || state.next_revision == 0
    {
        return Err(ProjectedMarketExecutionErrorV1::ProjectedState);
    }
    Ok(())
}

fn identity(value: [u8; 32]) -> Result<Identity, ProjectedMarketExecutionErrorV1> {
    Identity::new(value).map_err(|_| ProjectedMarketExecutionErrorV1::Projection)
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8, ProjectedMarketExecutionErrorV1> {
    input
        .get(offset)
        .copied()
        .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, ProjectedMarketExecutionErrorV1> {
    Ok(u16::from_le_bytes(
        slice(input, offset, 2)?
            .try_into()
            .map_err(|_| ProjectedMarketExecutionErrorV1::InvalidLength)?,
    ))
}

fn slice(
    input: &[u8],
    offset: usize,
    width: usize,
) -> Result<&[u8], ProjectedMarketExecutionErrorV1> {
    input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?,
        )
        .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)
}

fn put(
    output: &mut [u8],
    offset: usize,
    value: &[u8],
) -> Result<(), ProjectedMarketExecutionErrorV1> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?;
    let destination = output
        .get_mut(offset..end)
        .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)?;
    destination.copy_from_slice(value);
    Ok(())
}

fn put_u8(
    output: &mut [u8],
    offset: usize,
    value: u8,
) -> Result<(), ProjectedMarketExecutionErrorV1> {
    *output
        .get_mut(offset)
        .ok_or(ProjectedMarketExecutionErrorV1::InvalidLength)? = value;
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

    #[test]
    fn compact_wire_keeps_one_contiguous_family_request() {
        let core = [0x33; SERIES_CORE_REQUEST_BYTES_V1];
        let header = [0x44; PROJECTED_MARKET_FAMILY_HEADER_BYTES_V1];
        let witness = [0x55; 9 * PROJECTED_MARKET_WITNESS_WORD_BYTES_V1];
        let mut output = vec![0_u8; PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V1 + witness.len()];
        encode_projected_market_execution_v1(&mut output, &core, &header, &witness, 5)
            .expect("encode");
        let decoded = ProjectedMarketExecutionV1::decode(&output).expect("decode");
        assert_eq!(output.len(), 768);
        assert_eq!(decoded.witness_words(), 9);
        assert_eq!(decoded.affine_count(), 5);
        assert_eq!(decoded.core_request(), core);
        assert_eq!(
            decoded.family_request(),
            [&header[..], &witness[..]].concat()
        );
        assert_eq!(decoded.witness(), witness);
    }

    #[test]
    fn compact_wire_refuses_reserved_width_and_affine_substitution() {
        let core = [0x33; SERIES_CORE_REQUEST_BYTES_V1];
        let header = [0x44; PROJECTED_MARKET_FAMILY_HEADER_BYTES_V1];
        let witness = [0x55; PROJECTED_MARKET_WITNESS_WORD_BYTES_V1];
        let mut output = vec![0_u8; PROJECTED_MARKET_EXECUTION_FIXED_BYTES_V1 + witness.len()];
        encode_projected_market_execution_v1(&mut output, &core, &header, &witness, 1)
            .expect("encode");

        let mut changed = output.clone();
        *changed.get_mut(RESERVED_OFFSET).expect("reserved byte") = 1;
        assert_eq!(
            ProjectedMarketExecutionV1::decode(&changed),
            Err(ProjectedMarketExecutionErrorV1::NonCanonical)
        );
        changed = output.clone();
        *changed.get_mut(AFFINE_COUNT_OFFSET).expect("affine count") = 0;
        assert_eq!(
            ProjectedMarketExecutionV1::decode(&changed),
            Err(ProjectedMarketExecutionErrorV1::NonCanonical)
        );
        changed = output.clone();
        *changed
            .get_mut(WITNESS_WORDS_OFFSET)
            .expect("witness words") = 2;
        assert_eq!(
            ProjectedMarketExecutionV1::decode(&changed),
            Err(ProjectedMarketExecutionErrorV1::InvalidLength)
        );
        assert_eq!(
            ProjectedMarketExecutionV1::decode(
                output
                    .get(..output.len() - 1)
                    .expect("one-byte-short fixture"),
            ),
            Err(ProjectedMarketExecutionErrorV1::InvalidLength)
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
            Err(ProjectedMarketExecutionErrorV1::Projection)
        );

        changed = state;
        changed.phase = ProjectedCustodyPhaseV1::Initialized;
        assert_eq!(
            reconstruct_projected_lock_v1(changed, 88),
            Err(ProjectedMarketExecutionErrorV1::ProjectedState)
        );
        assert_eq!(
            reconstruct_projected_lock_v1(state, 0),
            Err(ProjectedMarketExecutionErrorV1::LockRequest)
        );
    }
}
