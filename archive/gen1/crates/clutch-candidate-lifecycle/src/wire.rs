// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Canonical version-two instruction intent wire format.
//!
//! Intent bytes never substitute for adapter authentication. In particular,
//! `Complete` names the expected verdict account; the adapter still executes
//! the bound relation and score policies and constructs the checked outcome.

use crate::codec::{CodecError, Reader, Writer};
use crate::state::{live, Error, Id, MAX_CANDIDATE_INDEX_PAGES};

pub const INTENT_MAGIC: u8 = 0xc7;
pub const INTENT_VERSION: u8 = 2;

const HEADER_BYTES: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateIntentV2 {
    Freeze {
        epoch: Id,
    },
    Begin {
        epoch: Id,
        candidate: Id,
        solver: Id,
        solver_reward_destination: Id,
        feed: Id,
        payer: Id,
        refund_destination: Id,
        expected_feed_bytes: u32,
        verification_units: u16,
    },
    Seal {
        epoch: Id,
        candidate: Id,
        feed: Id,
        content_digest: Id,
        verification_rent_principal: u64,
        work_reward_deposit: u64,
    },
    Progress {
        epoch: Id,
        candidate: Id,
        prior_units: u16,
        new_units: u16,
    },
    Complete {
        epoch: Id,
        candidate: Id,
        expected_verdict: Id,
    },
    Finalize {
        epoch: Id,
    },
    Expire {
        epoch: Id,
        candidate: Id,
    },
    MarkWorkClosed {
        epoch: Id,
        candidate: Id,
        observed_paid_units: u16,
    },
    ClaimBond {
        epoch: Id,
        candidate: Id,
    },
    ClaimWork {
        epoch: Id,
        candidate: Id,
    },
    CleanupCandidate {
        epoch: Id,
        candidate: Id,
    },
    ClaimSolver {
        epoch: Id,
        candidate: Id,
    },
    CloseIndexPage {
        epoch: Id,
        page_index: u8,
    },
    ClaimEpochUnused {
        epoch: Id,
    },
}

impl CandidateIntentV2 {
    pub const fn encoded_len(self) -> usize {
        match self {
            Self::Freeze { .. } | Self::Finalize { .. } | Self::ClaimEpochUnused { .. } => {
                HEADER_BYTES + 32
            }
            Self::Begin { .. } => HEADER_BYTES + (7 * 32) + 4 + 2,
            Self::Seal { .. } => HEADER_BYTES + (4 * 32) + (2 * 8),
            Self::Progress { .. } => HEADER_BYTES + (2 * 32) + (2 * 2),
            Self::Complete { .. } => HEADER_BYTES + (3 * 32),
            Self::Expire { .. }
            | Self::ClaimBond { .. }
            | Self::ClaimWork { .. }
            | Self::CleanupCandidate { .. }
            | Self::ClaimSolver { .. } => HEADER_BYTES + (2 * 32),
            Self::MarkWorkClosed { .. } => HEADER_BYTES + (2 * 32) + 2,
            Self::CloseIndexPage { .. } => HEADER_BYTES + 32 + 1,
        }
    }

    pub fn validate(self) -> Result<(), Error> {
        match self {
            Self::Freeze { epoch }
            | Self::Finalize { epoch }
            | Self::ClaimEpochUnused { epoch } => live(epoch),
            Self::CloseIndexPage { epoch, page_index } => {
                live(epoch)?;
                if usize::from(page_index) >= MAX_CANDIDATE_INDEX_PAGES {
                    return Err(Error::InvalidCount);
                }
                Ok(())
            }
            Self::Begin {
                epoch,
                candidate,
                solver,
                solver_reward_destination,
                feed,
                payer,
                refund_destination,
                expected_feed_bytes,
                verification_units,
            } => {
                for id in [
                    epoch,
                    candidate,
                    solver,
                    solver_reward_destination,
                    feed,
                    payer,
                    refund_destination,
                ] {
                    live(id)?;
                }
                if expected_feed_bytes == 0 || verification_units == 0 {
                    return Err(Error::InvalidCount);
                }
                Ok(())
            }
            Self::Seal {
                epoch,
                candidate,
                feed,
                content_digest,
                verification_rent_principal,
                work_reward_deposit,
            } => {
                for id in [epoch, candidate, feed, content_digest] {
                    live(id)?;
                }
                if verification_rent_principal == 0 || work_reward_deposit == 0 {
                    return Err(Error::Underfunded);
                }
                Ok(())
            }
            Self::Progress {
                epoch,
                candidate,
                prior_units,
                new_units,
            } => {
                live(epoch)?;
                live(candidate)?;
                if new_units <= prior_units {
                    return Err(Error::InvalidCount);
                }
                Ok(())
            }
            Self::Complete {
                epoch,
                candidate,
                expected_verdict,
            } => {
                live(epoch)?;
                live(candidate)?;
                live(expected_verdict)
            }
            Self::Expire { epoch, candidate }
            | Self::MarkWorkClosed {
                epoch, candidate, ..
            }
            | Self::ClaimBond { epoch, candidate }
            | Self::ClaimWork { epoch, candidate }
            | Self::CleanupCandidate { epoch, candidate }
            | Self::ClaimSolver { epoch, candidate } => {
                live(epoch)?;
                live(candidate)
            }
        }
    }

    pub fn encode(self, out: &mut [u8]) -> Result<(), CodecError> {
        self.validate().map_err(map_error)?;
        let mut writer = Writer::exact(out, self.encoded_len())?;
        writer.u8(INTENT_MAGIC)?;
        writer.u8(INTENT_VERSION)?;
        writer.u8(self.kind())?;
        match self {
            Self::Freeze { epoch }
            | Self::Finalize { epoch }
            | Self::ClaimEpochUnused { epoch } => write_id(&mut writer, epoch)?,
            Self::Begin {
                epoch,
                candidate,
                solver,
                solver_reward_destination,
                feed,
                payer,
                refund_destination,
                expected_feed_bytes,
                verification_units,
            } => {
                for id in [
                    epoch,
                    candidate,
                    solver,
                    solver_reward_destination,
                    feed,
                    payer,
                    refund_destination,
                ] {
                    write_id(&mut writer, id)?;
                }
                writer.bytes(&expected_feed_bytes.to_le_bytes())?;
                writer.u16(verification_units)?;
            }
            Self::Seal {
                epoch,
                candidate,
                feed,
                content_digest,
                verification_rent_principal,
                work_reward_deposit,
            } => {
                for id in [epoch, candidate, feed, content_digest] {
                    write_id(&mut writer, id)?;
                }
                writer.u64(verification_rent_principal)?;
                writer.u64(work_reward_deposit)?;
            }
            Self::Progress {
                epoch,
                candidate,
                prior_units,
                new_units,
            } => {
                write_id(&mut writer, epoch)?;
                write_id(&mut writer, candidate)?;
                writer.u16(prior_units)?;
                writer.u16(new_units)?;
            }
            Self::Complete {
                epoch,
                candidate,
                expected_verdict,
            } => {
                write_id(&mut writer, epoch)?;
                write_id(&mut writer, candidate)?;
                write_id(&mut writer, expected_verdict)?;
            }
            Self::Expire { epoch, candidate }
            | Self::ClaimBond { epoch, candidate }
            | Self::ClaimWork { epoch, candidate }
            | Self::CleanupCandidate { epoch, candidate }
            | Self::ClaimSolver { epoch, candidate } => {
                write_id(&mut writer, epoch)?;
                write_id(&mut writer, candidate)?;
            }
            Self::MarkWorkClosed {
                epoch,
                candidate,
                observed_paid_units,
            } => {
                write_id(&mut writer, epoch)?;
                write_id(&mut writer, candidate)?;
                writer.u16(observed_paid_units)?;
            }
            Self::CloseIndexPage { epoch, page_index } => {
                write_id(&mut writer, epoch)?;
                writer.u8(page_index)?;
            }
        }
        writer.finish()
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() < HEADER_BYTES {
            return Err(CodecError::WrongLength);
        }
        if input[0] != INTENT_MAGIC {
            return Err(CodecError::WrongTag);
        }
        if input[1] != INTENT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        let expected = encoded_len_for_kind(input[2])?;
        let mut reader = Reader::exact(input, expected)?;
        let _magic = reader.u8()?;
        let _version = reader.u8()?;
        let kind = reader.u8()?;
        let value = match kind {
            1 => Self::Freeze {
                epoch: read_id(&mut reader)?,
            },
            2 => Self::Begin {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
                solver: read_id(&mut reader)?,
                solver_reward_destination: read_id(&mut reader)?,
                feed: read_id(&mut reader)?,
                payer: read_id(&mut reader)?,
                refund_destination: read_id(&mut reader)?,
                expected_feed_bytes: u32::from_le_bytes(reader.array()?),
                verification_units: reader.u16()?,
            },
            3 => Self::Seal {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
                feed: read_id(&mut reader)?,
                content_digest: read_id(&mut reader)?,
                verification_rent_principal: reader.u64()?,
                work_reward_deposit: reader.u64()?,
            },
            4 => Self::Progress {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
                prior_units: reader.u16()?,
                new_units: reader.u16()?,
            },
            5 => Self::Complete {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
                expected_verdict: read_id(&mut reader)?,
            },
            6 => Self::Finalize {
                epoch: read_id(&mut reader)?,
            },
            7 => Self::Expire {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
            },
            8 => Self::MarkWorkClosed {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
                observed_paid_units: reader.u16()?,
            },
            9 => Self::ClaimBond {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
            },
            10 => Self::ClaimWork {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
            },
            11 => Self::CleanupCandidate {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
            },
            12 => Self::ClaimSolver {
                epoch: read_id(&mut reader)?,
                candidate: read_id(&mut reader)?,
            },
            13 => Self::CloseIndexPage {
                epoch: read_id(&mut reader)?,
                page_index: reader.u8()?,
            },
            14 => Self::ClaimEpochUnused {
                epoch: read_id(&mut reader)?,
            },
            _ => return Err(CodecError::InvalidEnum),
        };
        reader.finish()?;
        value.validate().map_err(map_error)?;
        Ok(value)
    }

    const fn kind(self) -> u8 {
        match self {
            Self::Freeze { .. } => 1,
            Self::Begin { .. } => 2,
            Self::Seal { .. } => 3,
            Self::Progress { .. } => 4,
            Self::Complete { .. } => 5,
            Self::Finalize { .. } => 6,
            Self::Expire { .. } => 7,
            Self::MarkWorkClosed { .. } => 8,
            Self::ClaimBond { .. } => 9,
            Self::ClaimWork { .. } => 10,
            Self::CleanupCandidate { .. } => 11,
            Self::ClaimSolver { .. } => 12,
            Self::CloseIndexPage { .. } => 13,
            Self::ClaimEpochUnused { .. } => 14,
        }
    }
}

fn encoded_len_for_kind(kind: u8) -> Result<usize, CodecError> {
    match kind {
        1 | 6 | 14 => Ok(HEADER_BYTES + 32),
        2 => Ok(HEADER_BYTES + (7 * 32) + 4 + 2),
        3 => Ok(HEADER_BYTES + (4 * 32) + (2 * 8)),
        4 => Ok(HEADER_BYTES + (2 * 32) + (2 * 2)),
        5 => Ok(HEADER_BYTES + (3 * 32)),
        7 | 9 | 10 | 11 | 12 => Ok(HEADER_BYTES + (2 * 32)),
        8 => Ok(HEADER_BYTES + (2 * 32) + 2),
        13 => Ok(HEADER_BYTES + 32 + 1),
        _ => Err(CodecError::InvalidEnum),
    }
}

fn write_id(writer: &mut Writer<'_>, id: Id) -> Result<(), CodecError> {
    writer.bytes(&id.bytes())
}

fn read_id(reader: &mut Reader<'_>) -> Result<Id, CodecError> {
    Ok(Id::from_bytes(reader.array()?))
}

fn map_error(error: Error) -> CodecError {
    match error {
        Error::ArithmeticOverflow => CodecError::ArithmeticOverflow,
        Error::ZeroIdentity => CodecError::ZeroIdentity,
        Error::InvalidCount | Error::CapacityReached => CodecError::InvalidCount,
        Error::InvalidState | Error::Replay | Error::NotActive => CodecError::InvalidEnum,
        Error::DuplicateIdentity
        | Error::InvalidPolicy
        | Error::InvalidSchedule
        | Error::MismatchedBinding
        | Error::Underfunded
        | Error::RankCollision
        | Error::UnresolvedCandidates => CodecError::MismatchedBinding,
    }
}
