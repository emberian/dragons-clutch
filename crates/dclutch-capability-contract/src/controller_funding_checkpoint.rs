//! Durable pre-Market coordination for controller-owned funding ledgers.
//!
//! A controller-funding preparation transaction creates the exact Resolution
//! and Trading subset ledgers before the projected-Custody staging transaction
//! runs. This fixed record is the sole bridge between those transactions. It
//! names the immutable founding, both authenticated ledger poststates, the
//! terminal Custody request, the canonical refund coordinates, and the expiry
//! after which preparation must roll back rather than open a Market.
//!
//! The checkpoint deliberately has no `OpenConsumed` persisted phase. Opening
//! consumes and closes the account atomically; the typed terminal receipt owns
//! that fact. The only durable phases are therefore `Prepared` and
//! `CustodyStaged`. Expiry cleanup has two distinct terminal paths: Prepared
//! cleanup never accepts Custody accounts, while CustodyStaged cleanup first
//! proves the exact Custody abort in the same transaction. Neither terminal
//! fact is a third persisted phase because the checkpoint is closed last.

use crate::Error;

/// Exact checkpoint account width.
pub const CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1: usize = 512;
/// Canonical checkpoint magic.
pub const CONTROLLER_FUNDING_CHECKPOINT_MAGIC_V1: [u8; 8] = *b"DCLTCFP1";
/// Implemented checkpoint schema version.
pub const CONTROLLER_FUNDING_CHECKPOINT_SCHEMA_V1: u16 = 1;
/// Trading PDA domain for one founding's controller-funding checkpoint.
pub const CONTROLLER_FUNDING_CHECKPOINT_PDA_DOMAIN_V1: &[u8] = b"dclutch/controller-funding/v1";
/// Domain for the ordered four-account Custody ladder poststate digest.
///
/// The adapter hashes this domain followed by, in
/// [`ControllerFundingCustodyLadderEntryV1`] order, each account's key, owner,
/// little-endian lamports, little-endian data length, and exact data.
pub const CONTROLLER_FUNDING_CUSTODY_LADDER_DIGEST_DOMAIN_V1: &[u8] =
    b"dclutch/controller-funding/custody-ladder/v1";
/// Exact number of accounts committed by the Custody ladder digest.
pub const CONTROLLER_FUNDING_CUSTODY_LADDER_ACCOUNT_COUNT_V1: usize = 4;

const SCHEMA_OFFSET: usize = 8;
const PHASE_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const HEADER_RESERVED_BYTES: usize = 5;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const GENERATION_OFFSET: usize = 80;
const MANIFEST_OFFSET: usize = 88;
const FUNDING_LIST_OFFSET: usize = 120;
const FOUND_REQUEST_DIGEST_OFFSET: usize = 152;
const PROJECT_FOUND_RECEIPT_DIGEST_OFFSET: usize = 184;
const RESOLUTION_LEDGER_OFFSET: usize = 216;
const RESOLUTION_LEDGER_DIGEST_OFFSET: usize = 248;
const TRADING_LEDGER_OFFSET: usize = 280;
const TRADING_LEDGER_DIGEST_OFFSET: usize = 312;
const FUNDING_SOURCE_OFFSET: usize = 344;
const RENT_CREDIT_OFFSET: usize = 376;
const LOCK_REQUEST_DIGEST_OFFSET: usize = 408;
const CUSTODY_LADDER_DIGEST_OFFSET: usize = 440;
const EXPIRY_SLOT_OFFSET: usize = 472;
const PREPARED_SLOT_OFFSET: usize = 480;
const STAGED_SLOT_OFFSET: usize = 488;
const RESOLUTION_MASK_OFFSET: usize = 496;
const TRADING_MASK_OFFSET: usize = 498;
const REVISION_OFFSET: usize = 500;
const BODY_RESERVED_OFFSET: usize = 508;
const BODY_RESERVED_BYTES: usize = 4;

const PREPARED_REVISION_V1: u64 = 1;
const CUSTODY_STAGED_REVISION_V1: u64 = 2;

/// Canonical order inside the Custody ladder poststate digest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ControllerFundingCustodyLadderEntryV1 {
    /// Projected-Custody replay account.
    ProjectedReplay = 0,
    /// Empty projected Hoard token vault.
    EmptyHoardVault = 1,
    /// Funded projected source-compartment token vault.
    FundedSourceVault = 2,
    /// Projected source-compartment replay account.
    SourceReplay = 3,
}

/// Terminal expiry path selected by the authenticated durable phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerFundingCheckpointAbortKindV1 {
    /// No Custody mutation finalized; close only the two Pending ledgers and checkpoint.
    PreparedExpired,
    /// Custody staging finalized; abort Custody first, then close the ledgers and checkpoint.
    CustodyStagedExpired,
}

/// Durable pre-Open lifecycle state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ControllerFundingCheckpointPhaseV1 {
    /// Both controller ledgers exist in their exact initial Pending state.
    Prepared = 1,
    /// Projected Custody staging finalized and committed its exact poststate.
    CustodyStaged = 2,
}

impl TryFrom<u8> for ControllerFundingCheckpointPhaseV1 {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Prepared),
            2 => Ok(Self::CustodyStaged),
            _ => Err(Error::UnknownControllerFundingCheckpointPhase),
        }
    }
}

/// All immutable and initial mutable facts needed to create a checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerFundingCheckpointInputV1 {
    /// Activated execution-release-set content identity.
    pub release_set: [u8; 32],
    /// Future Market address authenticated by Core's ProjectFound receipt.
    pub market: [u8; 32],
    /// Future Market generation.
    pub generation: u64,
    /// Immutable capability-manifest content identity.
    pub manifest: [u8; 32],
    /// Ordered two-ledger funding-list content identity.
    pub funding_list: [u8; 32],
    /// Digest of the exact generic founding request.
    pub found_request_digest: [u8; 32],
    /// Digest of Core's exact ProjectFound receipt bytes.
    pub project_found_receipt_digest: [u8; 32],
    /// Resolution-owned subset ledger address.
    pub resolution_ledger: [u8; 32],
    /// Exact initial Resolution ledger byte digest.
    pub resolution_ledger_digest: [u8; 32],
    /// Trading-owned subset ledger address.
    pub trading_ledger: [u8; 32],
    /// Exact initial Trading ledger byte digest.
    pub trading_ledger_digest: [u8; 32],
    /// Original signer and native-principal source.
    pub funding_source: [u8; 32],
    /// Canonical lifecycle RentCredit and dust/rent refund destination.
    pub rent_credit: [u8; 32],
    /// Digest of the exact terminal projected-Custody Lock request.
    pub lock_request_digest: [u8; 32],
    /// Last slot at which Custody staging and Market opening may proceed.
    pub expiry_slot: u64,
    /// Finalized slot that created both ledgers and this checkpoint.
    pub prepared_slot: u64,
    /// Exact Resolution-owned manifest subset.
    pub resolution_mask: u16,
    /// Exact Trading-owned manifest subset.
    pub trading_mask: u16,
}

/// Authenticated controller-funding checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerFundingCheckpointV1 {
    phase: ControllerFundingCheckpointPhaseV1,
    input: ControllerFundingCheckpointInputV1,
    custody_ladder_digest: [u8; 32],
    staged_slot: u64,
    revision: u64,
}

impl ControllerFundingCheckpointV1 {
    /// Create the one canonical Prepared checkpoint.
    pub fn prepared(input: ControllerFundingCheckpointInputV1) -> Result<Self, Error> {
        validate_input(input)?;
        Ok(Self {
            phase: ControllerFundingCheckpointPhaseV1::Prepared,
            input,
            custody_ladder_digest: [0; 32],
            staged_slot: 0,
            revision: PREPARED_REVISION_V1,
        })
    }

    /// Decode and validate the exact fixed checkpoint bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != CONTROLLER_FUNDING_CHECKPOINT_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, SCHEMA_OFFSET)? != CONTROLLER_FUNDING_CHECKPOINT_SCHEMA_V1 {
            return Err(Error::UnsupportedSchema);
        }
        require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
        require_zero(bytes, BODY_RESERVED_OFFSET, BODY_RESERVED_BYTES)?;
        let phase = ControllerFundingCheckpointPhaseV1::try_from(read_u8(bytes, PHASE_OFFSET)?)?;
        let input = ControllerFundingCheckpointInputV1 {
            release_set: read_array(bytes, RELEASE_SET_OFFSET)?,
            market: read_array(bytes, MARKET_OFFSET)?,
            generation: read_u64(bytes, GENERATION_OFFSET)?,
            manifest: read_array(bytes, MANIFEST_OFFSET)?,
            funding_list: read_array(bytes, FUNDING_LIST_OFFSET)?,
            found_request_digest: read_array(bytes, FOUND_REQUEST_DIGEST_OFFSET)?,
            project_found_receipt_digest: read_array(bytes, PROJECT_FOUND_RECEIPT_DIGEST_OFFSET)?,
            resolution_ledger: read_array(bytes, RESOLUTION_LEDGER_OFFSET)?,
            resolution_ledger_digest: read_array(bytes, RESOLUTION_LEDGER_DIGEST_OFFSET)?,
            trading_ledger: read_array(bytes, TRADING_LEDGER_OFFSET)?,
            trading_ledger_digest: read_array(bytes, TRADING_LEDGER_DIGEST_OFFSET)?,
            funding_source: read_array(bytes, FUNDING_SOURCE_OFFSET)?,
            rent_credit: read_array(bytes, RENT_CREDIT_OFFSET)?,
            lock_request_digest: read_array(bytes, LOCK_REQUEST_DIGEST_OFFSET)?,
            expiry_slot: read_u64(bytes, EXPIRY_SLOT_OFFSET)?,
            prepared_slot: read_u64(bytes, PREPARED_SLOT_OFFSET)?,
            resolution_mask: read_u16(bytes, RESOLUTION_MASK_OFFSET)?,
            trading_mask: read_u16(bytes, TRADING_MASK_OFFSET)?,
        };
        validate_input(input)?;
        let custody_ladder_digest = read_array(bytes, CUSTODY_LADDER_DIGEST_OFFSET)?;
        let staged_slot = read_u64(bytes, STAGED_SLOT_OFFSET)?;
        let revision = read_u64(bytes, REVISION_OFFSET)?;
        match phase {
            ControllerFundingCheckpointPhaseV1::Prepared
                if custody_ladder_digest != [0; 32]
                    || staged_slot != 0
                    || revision != PREPARED_REVISION_V1 =>
            {
                return Err(Error::InvalidControllerFundingCheckpointTransition);
            }
            ControllerFundingCheckpointPhaseV1::CustodyStaged
                if custody_ladder_digest == [0; 32]
                    || staged_slot < input.prepared_slot
                    || staged_slot > input.expiry_slot
                    || revision != CUSTODY_STAGED_REVISION_V1 =>
            {
                return Err(Error::InvalidControllerFundingCheckpointTransition);
            }
            _ => {}
        }
        Ok(Self {
            phase,
            input,
            custody_ladder_digest,
            staged_slot,
            revision,
        })
    }

    /// Encode the exact fixed checkpoint bytes.
    pub fn encode(self) -> [u8; CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1] {
        let mut output = [0_u8; CONTROLLER_FUNDING_CHECKPOINT_BYTES_V1];
        output[..8].copy_from_slice(&CONTROLLER_FUNDING_CHECKPOINT_MAGIC_V1);
        put_u16(
            &mut output,
            SCHEMA_OFFSET,
            CONTROLLER_FUNDING_CHECKPOINT_SCHEMA_V1,
        );
        output[PHASE_OFFSET] = self.phase as u8;
        put_array(&mut output, RELEASE_SET_OFFSET, self.input.release_set);
        put_array(&mut output, MARKET_OFFSET, self.input.market);
        put_u64(&mut output, GENERATION_OFFSET, self.input.generation);
        put_array(&mut output, MANIFEST_OFFSET, self.input.manifest);
        put_array(&mut output, FUNDING_LIST_OFFSET, self.input.funding_list);
        put_array(
            &mut output,
            FOUND_REQUEST_DIGEST_OFFSET,
            self.input.found_request_digest,
        );
        put_array(
            &mut output,
            PROJECT_FOUND_RECEIPT_DIGEST_OFFSET,
            self.input.project_found_receipt_digest,
        );
        put_array(
            &mut output,
            RESOLUTION_LEDGER_OFFSET,
            self.input.resolution_ledger,
        );
        put_array(
            &mut output,
            RESOLUTION_LEDGER_DIGEST_OFFSET,
            self.input.resolution_ledger_digest,
        );
        put_array(
            &mut output,
            TRADING_LEDGER_OFFSET,
            self.input.trading_ledger,
        );
        put_array(
            &mut output,
            TRADING_LEDGER_DIGEST_OFFSET,
            self.input.trading_ledger_digest,
        );
        put_array(
            &mut output,
            FUNDING_SOURCE_OFFSET,
            self.input.funding_source,
        );
        put_array(&mut output, RENT_CREDIT_OFFSET, self.input.rent_credit);
        put_array(
            &mut output,
            LOCK_REQUEST_DIGEST_OFFSET,
            self.input.lock_request_digest,
        );
        put_array(
            &mut output,
            CUSTODY_LADDER_DIGEST_OFFSET,
            self.custody_ladder_digest,
        );
        put_u64(&mut output, EXPIRY_SLOT_OFFSET, self.input.expiry_slot);
        put_u64(&mut output, PREPARED_SLOT_OFFSET, self.input.prepared_slot);
        put_u64(&mut output, STAGED_SLOT_OFFSET, self.staged_slot);
        put_u16(
            &mut output,
            RESOLUTION_MASK_OFFSET,
            self.input.resolution_mask,
        );
        put_u16(&mut output, TRADING_MASK_OFFSET, self.input.trading_mask);
        put_u64(&mut output, REVISION_OFFSET, self.revision);
        output
    }

    /// Advance Prepared to CustodyStaged exactly once.
    pub fn stage_custody(
        self,
        staged_slot: u64,
        custody_ladder_digest: [u8; 32],
    ) -> Result<Self, Error> {
        if self.phase != ControllerFundingCheckpointPhaseV1::Prepared
            || self.revision != PREPARED_REVISION_V1
            || custody_ladder_digest == [0; 32]
            || staged_slot < self.input.prepared_slot
            || staged_slot > self.input.expiry_slot
        {
            return Err(Error::InvalidControllerFundingCheckpointTransition);
        }
        Ok(Self {
            phase: ControllerFundingCheckpointPhaseV1::CustodyStaged,
            input: self.input,
            custody_ladder_digest,
            staged_slot,
            revision: CUSTODY_STAGED_REVISION_V1,
        })
    }

    /// Require that Open may consume this staged checkpoint at `current_slot`.
    pub fn authenticate_open_consumption(
        self,
        current_slot: u64,
        custody_ladder_digest: [u8; 32],
    ) -> Result<(), Error> {
        if self.phase != ControllerFundingCheckpointPhaseV1::CustodyStaged
            || self.revision != CUSTODY_STAGED_REVISION_V1
            || current_slot < self.staged_slot
            || current_slot > self.input.expiry_slot
            || custody_ladder_digest != self.custody_ladder_digest
        {
            return Err(Error::InvalidControllerFundingCheckpointTransition);
        }
        Ok(())
    }

    /// Require that expiry rollback, and not Open, owns the next transition.
    pub fn authenticate_expiry_abort(
        self,
        current_slot: u64,
    ) -> Result<ControllerFundingCheckpointAbortKindV1, Error> {
        if current_slot <= self.input.expiry_slot {
            return Err(Error::ControllerFundingCheckpointNotExpired);
        }
        match (self.phase, self.revision) {
            (ControllerFundingCheckpointPhaseV1::Prepared, PREPARED_REVISION_V1) => {
                Ok(ControllerFundingCheckpointAbortKindV1::PreparedExpired)
            }
            (ControllerFundingCheckpointPhaseV1::CustodyStaged, CUSTODY_STAGED_REVISION_V1) => {
                Ok(ControllerFundingCheckpointAbortKindV1::CustodyStagedExpired)
            }
            _ => Err(Error::InvalidControllerFundingCheckpointTransition),
        }
    }

    /// Return the durable phase.
    pub const fn phase(self) -> ControllerFundingCheckpointPhaseV1 {
        self.phase
    }

    /// Return all immutable preparation facts.
    pub const fn input(self) -> ControllerFundingCheckpointInputV1 {
        self.input
    }

    /// Borrow all immutable preparation facts without copying the fixed-width
    /// checkpoint body.
    ///
    /// SBF adapters use this accessor while authenticating a live checkpoint:
    /// keeping the 384-byte input in its semantic owner's storage avoids a
    /// second stack-resident copy at the verifier boundary.
    pub const fn input_ref(&self) -> &ControllerFundingCheckpointInputV1 {
        &self.input
    }

    /// Return the ordered four-account Custody ladder digest, or zero while Prepared.
    pub const fn custody_ladder_digest(self) -> [u8; 32] {
        self.custody_ladder_digest
    }

    /// Return the staging slot, or zero while Prepared.
    pub const fn staged_slot(self) -> u64 {
        self.staged_slot
    }

    /// Return the exact transition revision (`1` Prepared, `2` CustodyStaged).
    pub const fn revision(self) -> u64 {
        self.revision
    }
}

/// Canonical PDA seed projection for one checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerFundingCheckpointDerivationV1 {
    release_set: [u8; 32],
    market: [u8; 32],
    generation_le: [u8; 8],
    manifest: [u8; 32],
    funding_list: [u8; 32],
}

impl ControllerFundingCheckpointDerivationV1 {
    /// Bind one activated release set, future Market generation, and manifest.
    pub fn new(
        release_set: [u8; 32],
        market: [u8; 32],
        generation: u64,
        manifest: [u8; 32],
        funding_list: [u8; 32],
    ) -> Result<Self, Error> {
        for value in [release_set, market, manifest, funding_list] {
            require_nonzero(value)?;
        }
        Ok(Self {
            release_set,
            market,
            generation_le: generation.to_le_bytes(),
            manifest,
            funding_list,
        })
    }

    /// Return the exact ordered Solana PDA seed components.
    pub fn seed_components(&self) -> [&[u8]; 6] {
        [
            CONTROLLER_FUNDING_CHECKPOINT_PDA_DOMAIN_V1,
            self.release_set.as_slice(),
            self.market.as_slice(),
            self.generation_le.as_slice(),
            self.manifest.as_slice(),
            self.funding_list.as_slice(),
        ]
    }
}

fn validate_input(input: ControllerFundingCheckpointInputV1) -> Result<(), Error> {
    for value in [
        input.release_set,
        input.market,
        input.manifest,
        input.funding_list,
        input.found_request_digest,
        input.project_found_receipt_digest,
        input.resolution_ledger,
        input.resolution_ledger_digest,
        input.trading_ledger,
        input.trading_ledger_digest,
        input.funding_source,
        input.rent_credit,
        input.lock_request_digest,
    ] {
        require_nonzero(value)?;
    }
    if input.resolution_ledger == input.trading_ledger
        || input.funding_source == input.rent_credit
        || input.resolution_mask == 0
        || input.trading_mask == 0
        || input.resolution_mask & input.trading_mask != 0
        || input.expiry_slot <= input.prepared_slot
    {
        return Err(Error::InvalidControllerFundingCheckpointTransition);
    }
    Ok(())
}

fn require_nonzero(value: [u8; 32]) -> Result<(), Error> {
    if value == [0; 32] {
        return Err(Error::ZeroIdentifier);
    }
    Ok(())
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], Error> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u8(bytes: &[u8], offset: usize) -> Result<u8, Error> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<(), Error> {
    if bytes
        .get(offset..offset.checked_add(width).ok_or(Error::ArithmeticOverflow)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReservedBytes);
    }
    Ok(())
}

fn put_array<const N: usize>(output: &mut [u8], offset: usize, value: [u8; N]) {
    output[offset..offset + N].copy_from_slice(&value);
}

fn put_u16(output: &mut [u8], offset: usize, value: u16) {
    put_array(output, offset, value.to_le_bytes());
}

fn put_u64(output: &mut [u8], offset: usize, value: u64) {
    put_array(output, offset, value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ControllerFundingCheckpointInputV1 {
        ControllerFundingCheckpointInputV1 {
            release_set: [1; 32],
            market: [2; 32],
            generation: 3,
            manifest: [4; 32],
            funding_list: [5; 32],
            found_request_digest: [6; 32],
            project_found_receipt_digest: [7; 32],
            resolution_ledger: [8; 32],
            resolution_ledger_digest: [9; 32],
            trading_ledger: [10; 32],
            trading_ledger_digest: [11; 32],
            funding_source: [12; 32],
            rent_credit: [13; 32],
            lock_request_digest: [14; 32],
            expiry_slot: 90,
            prepared_slot: 40,
            resolution_mask: 0b1110,
            trading_mask: 0b0001,
        }
    }

    #[test]
    fn prepared_and_staged_round_trip_exactly() {
        let prepared = ControllerFundingCheckpointV1::prepared(input()).expect("prepared");
        assert_eq!(
            ControllerFundingCheckpointV1::decode(&prepared.encode()),
            Ok(prepared)
        );
        let staged = prepared.stage_custody(50, [15; 32]).expect("staged");
        assert_eq!(
            ControllerFundingCheckpointV1::decode(&staged.encode()),
            Ok(staged)
        );
        assert_eq!(staged.authenticate_open_consumption(60, [15; 32]), Ok(()));
        assert_eq!(prepared.revision(), PREPARED_REVISION_V1);
        assert_eq!(staged.revision(), CUSTODY_STAGED_REVISION_V1);
        assert_eq!(staged.custody_ladder_digest(), [15; 32]);
        assert_eq!(
            prepared.authenticate_expiry_abort(91),
            Ok(ControllerFundingCheckpointAbortKindV1::PreparedExpired)
        );
        assert_eq!(
            staged.authenticate_expiry_abort(91),
            Ok(ControllerFundingCheckpointAbortKindV1::CustodyStagedExpired)
        );
    }

    #[test]
    fn phase_time_digest_and_reserved_substitutions_refuse() {
        let prepared = ControllerFundingCheckpointV1::prepared(input()).expect("prepared");
        assert!(prepared.stage_custody(39, [15; 32]).is_err());
        assert!(prepared.stage_custody(91, [15; 32]).is_err());
        assert!(prepared.stage_custody(50, [0; 32]).is_err());
        assert!(
            prepared
                .authenticate_open_consumption(50, [15; 32])
                .is_err()
        );
        assert!(prepared.authenticate_expiry_abort(90).is_err());
        let staged = prepared.stage_custody(50, [15; 32]).expect("staged");
        assert!(staged.authenticate_open_consumption(49, [15; 32]).is_err());
        assert!(staged.authenticate_open_consumption(91, [15; 32]).is_err());
        assert!(staged.authenticate_open_consumption(60, [16; 32]).is_err());

        let mut bytes = staged.encode();
        for (offset, value) in [
            (PHASE_OFFSET, 9),
            (HEADER_RESERVED_OFFSET, 1),
            (BODY_RESERVED_OFFSET, 1),
        ] {
            let old = bytes[offset];
            bytes[offset] = value;
            assert!(ControllerFundingCheckpointV1::decode(&bytes).is_err());
            bytes[offset] = old;
        }
        let old_revision = read_u64(&bytes, REVISION_OFFSET).expect("revision");
        put_u64(&mut bytes, REVISION_OFFSET, old_revision + 1);
        assert!(ControllerFundingCheckpointV1::decode(&bytes).is_err());
    }

    #[test]
    fn every_identity_mask_alias_and_expiry_axis_is_bound() {
        let honest = input();
        for index in 0..13 {
            let mut hostile = honest;
            match index {
                0 => hostile.release_set = [0; 32],
                1 => hostile.market = [0; 32],
                2 => hostile.manifest = [0; 32],
                3 => hostile.funding_list = [0; 32],
                4 => hostile.found_request_digest = [0; 32],
                5 => hostile.project_found_receipt_digest = [0; 32],
                6 => hostile.resolution_ledger = [0; 32],
                7 => hostile.resolution_ledger_digest = [0; 32],
                8 => hostile.trading_ledger = [0; 32],
                9 => hostile.trading_ledger_digest = [0; 32],
                10 => hostile.funding_source = [0; 32],
                11 => hostile.rent_credit = [0; 32],
                12 => hostile.lock_request_digest = [0; 32],
                _ => unreachable!(),
            }
            assert!(ControllerFundingCheckpointV1::prepared(hostile).is_err());
        }
        for hostile in [
            ControllerFundingCheckpointInputV1 {
                trading_ledger: honest.resolution_ledger,
                ..honest
            },
            ControllerFundingCheckpointInputV1 {
                rent_credit: honest.funding_source,
                ..honest
            },
            ControllerFundingCheckpointInputV1 {
                resolution_mask: 0,
                ..honest
            },
            ControllerFundingCheckpointInputV1 {
                trading_mask: 0,
                ..honest
            },
            ControllerFundingCheckpointInputV1 {
                trading_mask: 0b0010,
                resolution_mask: 0b0011,
                ..honest
            },
            ControllerFundingCheckpointInputV1 {
                expiry_slot: honest.prepared_slot,
                ..honest
            },
        ] {
            assert!(ControllerFundingCheckpointV1::prepared(hostile).is_err());
        }
    }

    #[test]
    fn derivation_moves_on_every_seed_axis() {
        let honest =
            ControllerFundingCheckpointDerivationV1::new([1; 32], [2; 32], 3, [4; 32], [5; 32])
                .expect("derivation");
        for hostile in [
            ControllerFundingCheckpointDerivationV1::new([6; 32], [2; 32], 3, [4; 32], [5; 32]),
            ControllerFundingCheckpointDerivationV1::new([1; 32], [6; 32], 3, [4; 32], [5; 32]),
            ControllerFundingCheckpointDerivationV1::new([1; 32], [2; 32], 4, [4; 32], [5; 32]),
            ControllerFundingCheckpointDerivationV1::new([1; 32], [2; 32], 3, [6; 32], [5; 32]),
            ControllerFundingCheckpointDerivationV1::new([1; 32], [2; 32], 3, [4; 32], [6; 32]),
        ] {
            assert_ne!(honest, hostile.expect("hostile derivation"));
        }
        for hostile in [
            ControllerFundingCheckpointDerivationV1::new([0; 32], [2; 32], 3, [4; 32], [5; 32]),
            ControllerFundingCheckpointDerivationV1::new([1; 32], [0; 32], 3, [4; 32], [5; 32]),
            ControllerFundingCheckpointDerivationV1::new([1; 32], [2; 32], 3, [0; 32], [5; 32]),
            ControllerFundingCheckpointDerivationV1::new([1; 32], [2; 32], 3, [4; 32], [0; 32]),
        ] {
            assert!(hostile.is_err());
        }
    }
}
