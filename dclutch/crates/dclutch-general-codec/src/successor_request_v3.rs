//! Exact 64-byte General successor request V3.
//!
//! V2 could encode the seven settlement actions and no collection or candidate
//! action. It carried one primary-state bump, one bump reserved exclusively for
//! settlement `Close`, and a byte required to remain zero. That is not enough
//! for `VerifyCandidateRow`, whose one atomic transition advances the Candidate
//! and verifier states and may create the terminal verified-candidate record.
//!
//! V3 is a wire break without a width increase. It preserves the action selector
//! at byte 10 and every settlement coordinate at its V2 offset. Byte 62 is named
//! the secondary-state bump for every action (and remains `Close`'s terminal
//! bump); byte 63 becomes the conditional result-state bump. Each action has one
//! exact grammar, so an unused coordinate must be zero rather than becoming an
//! unauthenticated extension point.

use super::{Action, Error, Result};
use crate::generated_general_controller_request_v3::{
    REQUEST_V3_ABI_VERSION, REQUEST_V3_ACTION_OFFSET, REQUEST_V3_BYTES,
    REQUEST_V3_EXECUTION_INDEX_OFFSET, REQUEST_V3_EXPECTED_REVISION_OFFSET, REQUEST_V3_MAGIC,
    REQUEST_V3_MANIFEST_ORDER_OFFSET, REQUEST_V3_PAGE_INDEX_OFFSET, REQUEST_V3_PRIMARY_BUMP_OFFSET,
    REQUEST_V3_RESULT_BUMP_OFFSET, REQUEST_V3_SECONDARY_BUMP_OFFSET, REQUEST_V3_SUBJECT_ID_OFFSET,
};

/// Exact V3 request width, unchanged from V2.
pub const CONTROLLER_REQUEST_BYTES_V3: usize = REQUEST_V3_BYTES;
/// Action selector offset consumed by the capability program set.
pub const CONTROLLER_REQUEST_ACTION_OFFSET_V3: usize = REQUEST_V3_ACTION_OFFSET;
/// Verifier-emitted settlement-manifest row ordinal.
pub const CONTROLLER_REQUEST_MANIFEST_ORDER_OFFSET_V3: usize = REQUEST_V3_MANIFEST_ORDER_OFFSET;
/// Primary local-state canonical-PDA bump witness.
pub const CONTROLLER_REQUEST_PRIMARY_BUMP_OFFSET_V3: usize = REQUEST_V3_PRIMARY_BUMP_OFFSET;
/// Secondary local-state canonical-PDA bump witness.
pub const CONTROLLER_REQUEST_SECONDARY_BUMP_OFFSET_V3: usize = REQUEST_V3_SECONDARY_BUMP_OFFSET;
/// Conditional result-state canonical-PDA bump witness.
pub const CONTROLLER_REQUEST_RESULT_BUMP_OFFSET_V3: usize = REQUEST_V3_RESULT_BUMP_OFFSET;

/// Versioned General action catalogue.
///
/// Tags 0-13 preserve [`Action`] exactly. Tag 14 is the named candidate/work
/// escrow close that the V1 catalogue omitted even though the semantic owner
/// already exposes `GeneralCandidateV1::close_out`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ControllerActionV3 {
    /// Submit an authenticated candidate for deterministic comparison.
    Consider = 0,
    /// Close selection around the current best valid submitted candidate.
    Freeze = 1,
    /// Initialize the streamed settlement cursor.
    InitializeSettlement = 2,
    /// Collect one exact candidate page.
    Collect = 3,
    /// Perform the sole complete-set mint, merge, or no-op.
    Materialize = 4,
    /// Distribute one exact candidate page.
    Distribute = 5,
    /// Route the exact quote remainder and enter terminal state.
    Close = 6,
    /// Open one batch window.
    OpenBatch = 7,
    /// Admit one signed order.
    PlaceOrder = 8,
    /// Cancel one live order.
    CancelOrder = 9,
    /// Make one batch's order set final.
    CloseBatch = 10,
    /// Submit one content-addressed candidate.
    SubmitCandidate = 11,
    /// Verify one candidate execution row.
    VerifyCandidateRow = 12,
    /// Return one order's residual escrow.
    ReleaseOrder = 13,
    /// Close one considered or expired candidate and distribute its exact work
    /// escrow and rent credits.
    CloseCandidate = 14,
}

impl ControllerActionV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Consider),
            1 => Ok(Self::Freeze),
            2 => Ok(Self::InitializeSettlement),
            3 => Ok(Self::Collect),
            4 => Ok(Self::Materialize),
            5 => Ok(Self::Distribute),
            6 => Ok(Self::Close),
            7 => Ok(Self::OpenBatch),
            8 => Ok(Self::PlaceOrder),
            9 => Ok(Self::CancelOrder),
            10 => Ok(Self::CloseBatch),
            11 => Ok(Self::SubmitCandidate),
            12 => Ok(Self::VerifyCandidateRow),
            13 => Ok(Self::ReleaseOrder),
            14 => Ok(Self::CloseCandidate),
            _ => Err(Error::UnknownTag),
        }
    }

    /// Return the exact action selected by this V3 request.
    #[must_use]
    pub const fn legacy(self) -> Option<Action> {
        match self {
            Self::Consider => Some(Action::Consider),
            Self::Freeze => Some(Action::Freeze),
            Self::InitializeSettlement => Some(Action::InitializeSettlement),
            Self::Collect => Some(Action::Collect),
            Self::Materialize => Some(Action::Materialize),
            Self::Distribute => Some(Action::Distribute),
            Self::Close => Some(Action::Close),
            Self::OpenBatch => Some(Action::OpenBatch),
            Self::PlaceOrder => Some(Action::PlaceOrder),
            Self::CancelOrder => Some(Action::CancelOrder),
            Self::CloseBatch => Some(Action::CloseBatch),
            Self::SubmitCandidate => Some(Action::SubmitCandidate),
            Self::VerifyCandidateRow => Some(Action::VerifyCandidateRow),
            Self::ReleaseOrder => Some(Action::ReleaseOrder),
            Self::CloseCandidate => Some(Action::CloseCandidate),
        }
    }

    /// Exact selector byte.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

impl From<Action> for ControllerActionV3 {
    fn from(value: Action) -> Self {
        // The V1 tags are an exact prefix of this enum and `Action` hostile
        // decoding already proves the input lies in 0..=13.
        match value {
            Action::Consider => Self::Consider,
            Action::Freeze => Self::Freeze,
            Action::InitializeSettlement => Self::InitializeSettlement,
            Action::Collect => Self::Collect,
            Action::Materialize => Self::Materialize,
            Action::Distribute => Self::Distribute,
            Action::Close => Self::Close,
            Action::OpenBatch => Self::OpenBatch,
            Action::PlaceOrder => Self::PlaceOrder,
            Action::CancelOrder => Self::CancelOrder,
            Action::CloseBatch => Self::CloseBatch,
            Action::SubmitCandidate => Self::SubmitCandidate,
            Action::VerifyCandidateRow => Self::VerifyCandidateRow,
            Action::ReleaseOrder => Self::ReleaseOrder,
            Action::CloseCandidate => Self::CloseCandidate,
        }
    }
}

/// Exact runtime-width General request spanning the fifteen-action V3 catalogue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControllerRequestV3 {
    /// Requested data-defined action.
    pub action: ControllerActionV3,
    /// Exact optimistic revision, or zero for actions whose record has no
    /// independent revision coordinate.
    pub expected_revision: u64,
    /// Action subject: Candidate, batch, or order identity. Absent only for
    /// selection `Freeze`.
    pub subject_id: Option<[u8; 32]>,
    /// Action-specific page or candidate coordinate.
    pub page_index: u32,
    /// Action-specific row coordinate inside the selected page.
    pub execution_index: u8,
    /// Row ordinal inside a verifier-emitted settlement manifest.
    pub manifest_order_index: u8,
    /// Primary local-state canonical bump witness.
    pub primary_state_bump: u8,
    /// Secondary local-state canonical bump witness. Settlement `Close` uses
    /// this for its terminal record.
    pub secondary_state_bump: u8,
    /// Conditional result-state canonical bump witness. Only
    /// `VerifyCandidateRow` may carry it.
    pub result_state_bump: u8,
}

impl ControllerRequestV3 {
    /// Hostile-decode one exact V3 request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != CONTROLLER_REQUEST_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if input.get(..8) != Some(REQUEST_V3_MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if read_u16(input, 8)? != REQUEST_V3_ABI_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if !zero_range(input, 12, 4)? {
            return Err(Error::NonCanonicalPadding);
        }
        let raw_subject = read_array32(input, REQUEST_V3_SUBJECT_ID_OFFSET)?;
        let value = Self {
            action: ControllerActionV3::decode(byte(input, CONTROLLER_REQUEST_ACTION_OFFSET_V3)?)?,
            manifest_order_index: byte(input, CONTROLLER_REQUEST_MANIFEST_ORDER_OFFSET_V3)?,
            expected_revision: read_u64(input, REQUEST_V3_EXPECTED_REVISION_OFFSET)?,
            subject_id: if raw_subject == [0; 32] {
                None
            } else {
                Some(raw_subject)
            },
            page_index: read_u32(input, REQUEST_V3_PAGE_INDEX_OFFSET)?,
            execution_index: byte(input, REQUEST_V3_EXECUTION_INDEX_OFFSET)?,
            primary_state_bump: byte(input, CONTROLLER_REQUEST_PRIMARY_BUMP_OFFSET_V3)?,
            secondary_state_bump: byte(input, CONTROLLER_REQUEST_SECONDARY_BUMP_OFFSET_V3)?,
            result_state_bump: byte(input, CONTROLLER_REQUEST_RESULT_BUMP_OFFSET_V3)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical V3 request.
    pub fn to_bytes(self) -> Result<[u8; CONTROLLER_REQUEST_BYTES_V3]> {
        self.validate()?;
        let mut output = [0_u8; CONTROLLER_REQUEST_BYTES_V3];
        put(&mut output, 0, &REQUEST_V3_MAGIC)?;
        put(&mut output, 8, &REQUEST_V3_ABI_VERSION.to_le_bytes())?;
        put_byte(
            &mut output,
            CONTROLLER_REQUEST_ACTION_OFFSET_V3,
            self.action.tag(),
        )?;
        put_byte(
            &mut output,
            CONTROLLER_REQUEST_MANIFEST_ORDER_OFFSET_V3,
            self.manifest_order_index,
        )?;
        put(
            &mut output,
            REQUEST_V3_EXPECTED_REVISION_OFFSET,
            &self.expected_revision.to_le_bytes(),
        )?;
        if let Some(subject) = self.subject_id {
            put(&mut output, REQUEST_V3_SUBJECT_ID_OFFSET, &subject)?;
        }
        put(
            &mut output,
            REQUEST_V3_PAGE_INDEX_OFFSET,
            &self.page_index.to_le_bytes(),
        )?;
        put_byte(
            &mut output,
            REQUEST_V3_EXECUTION_INDEX_OFFSET,
            self.execution_index,
        )?;
        put_byte(
            &mut output,
            CONTROLLER_REQUEST_PRIMARY_BUMP_OFFSET_V3,
            self.primary_state_bump,
        )?;
        put_byte(
            &mut output,
            CONTROLLER_REQUEST_SECONDARY_BUMP_OFFSET_V3,
            self.secondary_state_bump,
        )?;
        put_byte(
            &mut output,
            CONTROLLER_REQUEST_RESULT_BUMP_OFFSET_V3,
            self.result_state_bump,
        )?;
        Ok(output)
    }

    fn validate(self) -> Result<()> {
        if self.subject_id.as_ref().is_some_and(is_zero) {
            return Err(Error::ZeroCoordinate);
        }
        let no_manifest = !matches!(
            self.action,
            ControllerActionV3::Collect | ControllerActionV3::Distribute
        ) && self.manifest_order_index != 0;
        if no_manifest {
            return Err(Error::NonCanonicalPadding);
        }
        match self.action {
            ControllerActionV3::Freeze
                if self.subject_id.is_none()
                    && self.page_index == 0
                    && self.execution_index == 0
                    && self.secondary_state_bump == 0
                    && self.result_state_bump == 0 =>
            {
                Ok(())
            }
            ControllerActionV3::Consider
                if self.subject_id.is_some()
                    && self.execution_index == 0
                    && self.result_state_bump == 0 =>
            {
                Ok(())
            }
            ControllerActionV3::Collect | ControllerActionV3::Distribute
                if self.subject_id.is_some()
                    && self.secondary_state_bump == 0
                    && self.result_state_bump == 0 =>
            {
                Ok(())
            }
            ControllerActionV3::InitializeSettlement | ControllerActionV3::Materialize
                if self.subject_id.is_some()
                    && self.page_index == 0
                    && self.execution_index == 0
                    && self.secondary_state_bump == 0
                    && self.result_state_bump == 0 =>
            {
                Ok(())
            }
            ControllerActionV3::Close
                if self.subject_id.is_some()
                    && self.page_index == 0
                    && self.execution_index == 0
                    && self.result_state_bump == 0 =>
            {
                Ok(())
            }
            ControllerActionV3::OpenBatch | ControllerActionV3::CloseBatch
                if self.subject_id.is_some()
                    && self.page_index == 0
                    && self.execution_index == 0
                    && self.secondary_state_bump == 0
                    && self.result_state_bump == 0 =>
            {
                Ok(())
            }
            ControllerActionV3::PlaceOrder | ControllerActionV3::CancelOrder
                if self.subject_id.is_some()
                    && self.expected_revision == 0
                    && self.page_index == 0
                    && self.execution_index == 0
                    && self.result_state_bump == 0 =>
            {
                Ok(())
            }
            ControllerActionV3::SubmitCandidate
            | ControllerActionV3::ReleaseOrder
            | ControllerActionV3::CloseCandidate
                if self.subject_id.is_some()
                    && self.expected_revision == 0
                    && self.page_index == 0
                    && self.execution_index == 0
                    && self.secondary_state_bump == 0
                    && self.result_state_bump == 0 =>
            {
                Ok(())
            }
            ControllerActionV3::VerifyCandidateRow if self.subject_id.is_some() => Ok(()),
            _ => Err(Error::InvalidCursor),
        }
    }
}

fn byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array32(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    read_array(input, offset)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn zero_range(input: &[u8], offset: usize, width: usize) -> Result<bool> {
    Ok(input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|value| *value == 0))
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIONS: [ControllerActionV3; 15] = [
        ControllerActionV3::Consider,
        ControllerActionV3::Freeze,
        ControllerActionV3::InitializeSettlement,
        ControllerActionV3::Collect,
        ControllerActionV3::Materialize,
        ControllerActionV3::Distribute,
        ControllerActionV3::Close,
        ControllerActionV3::OpenBatch,
        ControllerActionV3::PlaceOrder,
        ControllerActionV3::CancelOrder,
        ControllerActionV3::CloseBatch,
        ControllerActionV3::SubmitCandidate,
        ControllerActionV3::VerifyCandidateRow,
        ControllerActionV3::ReleaseOrder,
        ControllerActionV3::CloseCandidate,
    ];

    fn request(action: ControllerActionV3) -> ControllerRequestV3 {
        let subject_id = (action != ControllerActionV3::Freeze).then_some([0x31; 32]);
        ControllerRequestV3 {
            action,
            expected_revision: if matches!(
                action,
                ControllerActionV3::PlaceOrder
                    | ControllerActionV3::CancelOrder
                    | ControllerActionV3::SubmitCandidate
                    | ControllerActionV3::ReleaseOrder
                    | ControllerActionV3::CloseCandidate
            ) {
                0
            } else {
                7
            },
            subject_id,
            page_index: if matches!(
                action,
                ControllerActionV3::Consider
                    | ControllerActionV3::Collect
                    | ControllerActionV3::Distribute
                    | ControllerActionV3::VerifyCandidateRow
            ) {
                2
            } else {
                0
            },
            execution_index: if matches!(
                action,
                ControllerActionV3::Collect
                    | ControllerActionV3::Distribute
                    | ControllerActionV3::VerifyCandidateRow
            ) {
                3
            } else {
                0
            },
            manifest_order_index: u8::from(matches!(
                action,
                ControllerActionV3::Collect | ControllerActionV3::Distribute
            )),
            primary_state_bump: 41,
            secondary_state_bump: if matches!(
                action,
                ControllerActionV3::Consider
                    | ControllerActionV3::Close
                    | ControllerActionV3::PlaceOrder
                    | ControllerActionV3::CancelOrder
                    | ControllerActionV3::VerifyCandidateRow
            ) {
                42
            } else {
                0
            },
            result_state_bump: if action == ControllerActionV3::VerifyCandidateRow {
                43
            } else {
                0
            },
        }
    }

    #[test]
    fn all_fifteen_actions_round_trip_one_exact_64_byte_grammar() {
        for action in ACTIONS {
            let value = request(action);
            let bytes = value.to_bytes().expect("canonical V3 request");
            assert_eq!(bytes.len(), CONTROLLER_REQUEST_BYTES_V3);
            assert_eq!(bytes[CONTROLLER_REQUEST_ACTION_OFFSET_V3], action.tag());
            assert_eq!(ControllerRequestV3::decode(&bytes), Ok(value));
        }
    }

    #[test]
    fn settlement_offsets_stay_packet_and_selector_compatible_with_v2() {
        use crate::successor_request_v2::{
            CONTROLLER_REQUEST_ACTION_OFFSET_V2, CONTROLLER_REQUEST_BYTES_V2,
            CONTROLLER_REQUEST_MANIFEST_ORDER_OFFSET_V2, CONTROLLER_REQUEST_STATE_BUMP_OFFSET_V2,
            CONTROLLER_REQUEST_TERMINAL_BUMP_OFFSET_V2,
        };

        assert_eq!(CONTROLLER_REQUEST_BYTES_V3, CONTROLLER_REQUEST_BYTES_V2);
        assert_eq!(
            CONTROLLER_REQUEST_ACTION_OFFSET_V3,
            CONTROLLER_REQUEST_ACTION_OFFSET_V2
        );
        assert_eq!(
            CONTROLLER_REQUEST_MANIFEST_ORDER_OFFSET_V3,
            CONTROLLER_REQUEST_MANIFEST_ORDER_OFFSET_V2
        );
        assert_eq!(
            CONTROLLER_REQUEST_PRIMARY_BUMP_OFFSET_V3,
            CONTROLLER_REQUEST_STATE_BUMP_OFFSET_V2
        );
        assert_eq!(
            CONTROLLER_REQUEST_SECONDARY_BUMP_OFFSET_V3,
            CONTROLLER_REQUEST_TERMINAL_BUMP_OFFSET_V2
        );
    }

    #[test]
    fn v2_has_no_legal_encoding_for_any_gen_seven_action() {
        use crate::successor_request_v2::ControllerRequestV2;

        for legacy_action in [
            Action::OpenBatch,
            Action::PlaceOrder,
            Action::CancelOrder,
            Action::CloseBatch,
            Action::SubmitCandidate,
            Action::VerifyCandidateRow,
            Action::ReleaseOrder,
        ] {
            let action = ControllerActionV3::from(legacy_action);
            let legacy = ControllerRequestV2 {
                action: legacy_action,
                expected_revision: 0,
                candidate_id: Some([0x31; 32]),
                page_index: 0,
                execution_index: 0,
                manifest_order_index: 0,
                state_bump: 0,
                terminal_record_bump: 0,
            };
            assert_eq!(
                legacy.to_bytes(),
                Err(Error::InvalidCursor),
                "V2 unexpectedly encoded {legacy_action:?}",
            );
            assert_eq!(action.legacy(), Some(legacy_action));
        }
    }

    #[test]
    fn hostile_mode_substitutions_and_unused_state_coordinates_refuse() {
        for (action, offset) in [
            (
                ControllerActionV3::OpenBatch,
                CONTROLLER_REQUEST_SECONDARY_BUMP_OFFSET_V3,
            ),
            (
                ControllerActionV3::PlaceOrder,
                CONTROLLER_REQUEST_RESULT_BUMP_OFFSET_V3,
            ),
            (
                ControllerActionV3::SubmitCandidate,
                CONTROLLER_REQUEST_SECONDARY_BUMP_OFFSET_V3,
            ),
            (
                ControllerActionV3::ReleaseOrder,
                CONTROLLER_REQUEST_RESULT_BUMP_OFFSET_V3,
            ),
        ] {
            let mut bytes = request(action).to_bytes().expect("canonical");
            bytes[offset] = 1;
            assert_eq!(
                ControllerRequestV3::decode(&bytes),
                Err(Error::InvalidCursor),
                "{action:?} accepted an unused state coordinate",
            );
        }

        let mut settle_as_verify = request(ControllerActionV3::Collect)
            .to_bytes()
            .expect("canonical settlement row");
        settle_as_verify[CONTROLLER_REQUEST_ACTION_OFFSET_V3] =
            ControllerActionV3::VerifyCandidateRow.tag();
        assert_eq!(
            ControllerRequestV3::decode(&settle_as_verify),
            Err(Error::NonCanonicalPadding),
        );

        let mut verify_as_settle = request(ControllerActionV3::VerifyCandidateRow)
            .to_bytes()
            .expect("canonical verification row");
        verify_as_settle[CONTROLLER_REQUEST_ACTION_OFFSET_V3] = ControllerActionV3::Collect.tag();
        assert_eq!(
            ControllerRequestV3::decode(&verify_as_settle),
            Err(Error::InvalidCursor),
        );
    }

    #[test]
    fn zero_subject_and_noncanonical_revision_refuse() {
        let mut zero_subject = request(ControllerActionV3::PlaceOrder);
        zero_subject.subject_id = Some([0; 32]);
        assert_eq!(zero_subject.to_bytes(), Err(Error::ZeroCoordinate));

        for action in [
            ControllerActionV3::PlaceOrder,
            ControllerActionV3::CancelOrder,
            ControllerActionV3::SubmitCandidate,
            ControllerActionV3::ReleaseOrder,
            ControllerActionV3::CloseCandidate,
        ] {
            let mut noncanonical = request(action);
            noncanonical.expected_revision = 1;
            assert_eq!(noncanonical.to_bytes(), Err(Error::InvalidCursor));
        }
    }
}
