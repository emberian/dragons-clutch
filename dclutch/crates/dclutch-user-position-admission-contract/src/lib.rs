//! Wallet-authorized outer contract for one canonical Claims User Position.
//!
//! Claims owns Position state, admission evidence, and every Market/Product/
//! release/rent check. This crate owns only the narrower public caller: an
//! ordinary wallet authorizes occupying or reclaiming its unique Position
//! coordinate, while Trading supplies the release-bound caller PDA Claims
//! already requires.
//!
//! The outer frame deliberately carries the Claims program twice. Coordinate
//! zero is the CPI callee supplied to the runtime; child coordinate 19 is part
//! of Claims' authenticated 26-account frame. They must alias on execution.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub use dclutch_claims_svm::protocol_position_v2::{
    PROTOCOL_POSITION_ADMISSION_BYTES_V2, PROTOCOL_POSITION_CLOSE_RECEIPT_BYTES_V2,
    PROTOCOL_POSITION_REQUEST_BYTES_V2, ProtocolPositionActionV2, ProtocolPositionAdmissionV2,
    ProtocolPositionCloseReceiptV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2,
};

use dclutch_claims_svm::frame_spec_v1::ClaimsFrameSpecV1;

/// Exact outer selector retained from the admission-only first release.
///
/// The embedded Claims action now selects either admission or close; changing
/// this selector would create a second public authority for the same Position
/// lifecycle.
pub const USER_POSITION_ADMISSION_MAGIC_V1: [u8; 8] = *b"DCLTPUA1";
/// Exact outer request width: selector followed by one canonical Claims request.
pub const USER_POSITION_ADMISSION_REQUEST_BYTES_V1: usize =
    USER_POSITION_ADMISSION_MAGIC_V1.len() + PROTOCOL_POSITION_REQUEST_BYTES_V2;
/// Claims callee coordinate before the forwarded child frame.
pub const USER_POSITION_ADMISSION_CLAIMS_CALLEE_ACCOUNT_V1: usize = 0;
/// First outer coordinate forwarded as child coordinate zero.
pub const USER_POSITION_ADMISSION_CHILD_ACCOUNT_OFFSET_V1: usize = 1;
/// Exact child account count selected by `Admit`.
pub const USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1: usize = 26;
/// Exact outer account count.
pub const USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1: usize =
    USER_POSITION_ADMISSION_CHILD_ACCOUNT_OFFSET_V1
        + USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1;
/// Outer coordinate of the Trading caller-authority PDA.
pub const USER_POSITION_ADMISSION_AUTHORITY_ACCOUNT_V1: usize = 1;
/// Outer coordinate of the Trading program alias inside the Claims frame.
pub const USER_POSITION_ADMISSION_TRADING_PROGRAM_ACCOUNT_V1: usize = 18;
/// Outer coordinate of the Claims program alias inside the Claims frame.
pub const USER_POSITION_ADMISSION_CLAIMS_PROGRAM_ACCOUNT_V1: usize = 20;
/// Outer coordinate of the wallet identity authorizing admission.
pub const USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1: usize = 24;

/// Claims callee coordinate before the forwarded close child frame.
pub const USER_POSITION_CLOSE_CLAIMS_CALLEE_ACCOUNT_V1: usize = 0;
/// First outer coordinate forwarded as close child coordinate zero.
pub const USER_POSITION_CLOSE_CHILD_ACCOUNT_OFFSET_V1: usize = 1;
/// Exact child account count selected by `Close`.
pub const USER_POSITION_CLOSE_CHILD_ACCOUNT_COUNT_V1: usize = 15;
/// Exact outer account count selected by `Close`.
pub const USER_POSITION_CLOSE_ACCOUNT_COUNT_V1: usize =
    USER_POSITION_CLOSE_CHILD_ACCOUNT_OFFSET_V1 + USER_POSITION_CLOSE_CHILD_ACCOUNT_COUNT_V1;
/// Outer close coordinate of the Trading caller-authority PDA.
pub const USER_POSITION_CLOSE_AUTHORITY_ACCOUNT_V1: usize = 1;
/// Outer close coordinate of the Trading program alias inside the Claims frame.
pub const USER_POSITION_CLOSE_TRADING_PROGRAM_ACCOUNT_V1: usize = 9;
/// Outer close coordinate of the Claims program alias inside the Claims frame.
pub const USER_POSITION_CLOSE_CLAIMS_PROGRAM_ACCOUNT_V1: usize = 11;
/// Outer close coordinate of the wallet identity authorizing reclamation.
pub const USER_POSITION_CLOSE_OWNER_ACCOUNT_V1: usize = 13;

const _: () = assert!(USER_POSITION_ADMISSION_CHILD_ACCOUNT_COUNT_V1 == 26);
const _: () = assert!(USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1 == 27);
const _: () = assert!(USER_POSITION_CLOSE_CHILD_ACCOUNT_COUNT_V1 == 15);
const _: () = assert!(USER_POSITION_CLOSE_ACCOUNT_COUNT_V1 == 16);

/// Stable hostile-decode or frame-spec refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserPositionAdmissionErrorV1 {
    /// The outer request did not have its one exact width.
    InvalidLength,
    /// The outer selector was not `DCLTPUA1`.
    InvalidMagic,
    /// The embedded Claims request refused canonical decoding.
    InvalidClaimsRequest,
    /// The embedded request selected an unsupported lifecycle action.
    InvalidAction,
    /// The embedded request selected a non-wallet owner kind.
    InvalidOwnerKind,
    /// The embedded request did not describe vacant state.
    InvalidPresence,
    /// The queried outer account coordinate was outside the exact frame.
    InvalidAccountCoordinate,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, UserPositionAdmissionErrorV1>;

/// One exact wallet-authorized admission or close request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserPositionAdmissionRequestV1 {
    claims_request: ProtocolPositionRequestV2,
}

impl UserPositionAdmissionRequestV1 {
    /// Select one canonical Claims `User` Position lifecycle request.
    pub fn new(claims_request: ProtocolPositionRequestV2) -> Result<Self> {
        // Re-encode first so callers cannot construct a value that the Claims
        // semantic owner itself would refuse.
        claims_request
            .to_bytes()
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidClaimsRequest)?;
        if claims_request.owner_kind != ProtocolPositionOwnerKindV2::User {
            return Err(UserPositionAdmissionErrorV1::InvalidOwnerKind);
        }
        match (claims_request.action, claims_request.presence) {
            (ProtocolPositionActionV2::Admit, ProtocolPositionPresenceV2::Vacant)
            | (ProtocolPositionActionV2::Close, ProtocolPositionPresenceV2::Existing) => {}
            _ => return Err(UserPositionAdmissionErrorV1::InvalidPresence),
        }
        Ok(Self { claims_request })
    }

    /// Decode one exact outer request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != USER_POSITION_ADMISSION_REQUEST_BYTES_V1 {
            return Err(UserPositionAdmissionErrorV1::InvalidLength);
        }
        if input.get(..USER_POSITION_ADMISSION_MAGIC_V1.len())
            != Some(USER_POSITION_ADMISSION_MAGIC_V1.as_slice())
        {
            return Err(UserPositionAdmissionErrorV1::InvalidMagic);
        }
        let child = input
            .get(USER_POSITION_ADMISSION_MAGIC_V1.len()..)
            .ok_or(UserPositionAdmissionErrorV1::InvalidLength)?;
        let claims_request = ProtocolPositionRequestV2::decode(child)
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidClaimsRequest)?;
        Self::new(claims_request)
    }

    /// Encode the exact selector and canonical embedded Claims request.
    pub fn to_bytes(self) -> Result<[u8; USER_POSITION_ADMISSION_REQUEST_BYTES_V1]> {
        Self::new(self.claims_request)?;
        let child = self
            .claims_request
            .to_bytes()
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidClaimsRequest)?;
        let mut output = [0_u8; USER_POSITION_ADMISSION_REQUEST_BYTES_V1];
        let (magic, request) = output.split_at_mut(USER_POSITION_ADMISSION_MAGIC_V1.len());
        magic.copy_from_slice(&USER_POSITION_ADMISSION_MAGIC_V1);
        request.copy_from_slice(&child);
        Ok(output)
    }

    /// Embedded canonical Claims request.
    #[must_use]
    pub const fn claims_request(self) -> ProtocolPositionRequestV2 {
        self.claims_request
    }

    /// Encode only the exact child request supplied to Claims.
    pub fn claims_request_bytes(self) -> Result<[u8; PROTOCOL_POSITION_REQUEST_BYTES_V2]> {
        Self::new(self.claims_request)?;
        self.claims_request
            .to_bytes()
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidClaimsRequest)
    }

    /// Require an immediate Claims receipt to bind this exact child request
    /// and the two programs on the CPI boundary.
    pub fn validate_claims_receipt(
        self,
        receipt: &[u8],
        request_digest: [u8; 32],
        claims_program: [u8; 32],
        trading_program: [u8; 32],
    ) -> Result<ProtocolPositionAdmissionV2> {
        if self.claims_request.action != ProtocolPositionActionV2::Admit {
            return Err(UserPositionAdmissionErrorV1::InvalidAction);
        }
        let receipt = ProtocolPositionAdmissionV2::decode_receipt(receipt)
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidClaimsRequest)?;
        receipt
            .validate_request(
                self.claims_request,
                request_digest,
                claims_program,
                trading_program,
            )
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidClaimsRequest)?;
        Ok(receipt)
    }

    /// Require an immediate Claims close receipt to bind this exact child
    /// request and Claims program on the CPI boundary.
    pub fn validate_close_claims_receipt(
        self,
        receipt: &[u8],
        request_digest: [u8; 32],
        claims_program: [u8; 32],
    ) -> Result<ProtocolPositionCloseReceiptV2> {
        if self.claims_request.action != ProtocolPositionActionV2::Close {
            return Err(UserPositionAdmissionErrorV1::InvalidAction);
        }
        let receipt = ProtocolPositionCloseReceiptV2::decode(receipt)
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidClaimsRequest)?;
        receipt
            .validate_request(self.claims_request, request_digest, claims_program)
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidClaimsRequest)?;
        Ok(receipt)
    }
}

/// Whether bytes select this outer, including malformed requests that must
/// refuse here rather than falling through to another Trading ABI.
#[must_use]
pub fn is_user_position_admission_v1(input: &[u8]) -> bool {
    input.get(..USER_POSITION_ADMISSION_MAGIC_V1.len())
        == Some(USER_POSITION_ADMISSION_MAGIC_V1.as_slice())
}

/// Exact top-level privilege required at one outer account coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserPositionAdmissionPrivilegesV1 {
    signer: bool,
    writable: bool,
    executable: bool,
}

impl UserPositionAdmissionPrivilegesV1 {
    /// Whether the top-level transaction must mark the account as a signer.
    #[must_use]
    pub const fn signer(self) -> bool {
        self.signer
    }

    /// Whether the outer instruction must grant writable privilege.
    #[must_use]
    pub const fn writable(self) -> bool {
        self.writable
    }

    /// Whether the account must be executable.
    #[must_use]
    pub const fn executable(self) -> bool {
        self.executable
    }
}

/// Exact 27-account outer frame specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserPositionAdmissionFrameV1;

impl UserPositionAdmissionFrameV1 {
    /// Exact outer account count.
    #[must_use]
    pub const fn account_count(self) -> usize {
        USER_POSITION_ADMISSION_ACCOUNT_COUNT_V1
    }

    /// Exact top-level privilege tuple at `index`.
    pub fn privileges(self, index: usize) -> Result<UserPositionAdmissionPrivilegesV1> {
        if index == USER_POSITION_ADMISSION_CLAIMS_CALLEE_ACCOUNT_V1 {
            return Ok(UserPositionAdmissionPrivilegesV1 {
                signer: false,
                writable: false,
                executable: true,
            });
        }
        let child_index = index
            .checked_sub(USER_POSITION_ADMISSION_CHILD_ACCOUNT_OFFSET_V1)
            .ok_or(UserPositionAdmissionErrorV1::InvalidAccountCoordinate)?;
        let child_index = u16::try_from(child_index)
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidAccountCoordinate)?;
        let child = ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Admit)
            .account(child_index)
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidAccountCoordinate)?
            .privileges();
        // The PDA signer exists only in the Claims CPI; the wallet signature
        // exists only on the outer. All other privilege bits are identical.
        let signer = index == USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1;
        Ok(UserPositionAdmissionPrivilegesV1 {
            signer,
            writable: child.writable(),
            executable: child.executable(),
        })
    }
}

/// Exact 16-account wallet-authorized close frame specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserPositionCloseFrameV1;

impl UserPositionCloseFrameV1 {
    /// Exact outer account count.
    #[must_use]
    pub const fn account_count(self) -> usize {
        USER_POSITION_CLOSE_ACCOUNT_COUNT_V1
    }

    /// Exact top-level privilege tuple at `index`.
    pub fn privileges(self, index: usize) -> Result<UserPositionAdmissionPrivilegesV1> {
        if index == USER_POSITION_CLOSE_CLAIMS_CALLEE_ACCOUNT_V1 {
            return Ok(UserPositionAdmissionPrivilegesV1 {
                signer: false,
                writable: false,
                executable: true,
            });
        }
        let child_index = index
            .checked_sub(USER_POSITION_CLOSE_CHILD_ACCOUNT_OFFSET_V1)
            .ok_or(UserPositionAdmissionErrorV1::InvalidAccountCoordinate)?;
        let child_index = u16::try_from(child_index)
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidAccountCoordinate)?;
        let child = ClaimsFrameSpecV1::protocol_position(ProtocolPositionActionV2::Close)
            .account(child_index)
            .map_err(|_| UserPositionAdmissionErrorV1::InvalidAccountCoordinate)?
            .privileges();
        // The request-bound PDA signs only inside the Claims CPI. The wallet
        // authorizes close only on the outer and remains readonly to Claims.
        let signer = index == USER_POSITION_CLOSE_OWNER_ACCOUNT_V1;
        Ok(UserPositionAdmissionPrivilegesV1 {
            signer,
            writable: child.writable(),
            executable: child.executable(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claims_request() -> ProtocolPositionRequestV2 {
        ProtocolPositionRequestV2 {
            action: ProtocolPositionActionV2::Admit,
            owner_kind: ProtocolPositionOwnerKindV2::User,
            presence: ProtocolPositionPresenceV2::Vacant,
            release_set: [1; 32],
            market: [2; 32],
            position_owner: [3; 32],
            parent_request_digest: [4; 32],
            rent_credit: [5; 32],
            rent_program: [6; 32],
            generation: 7,
            expected_market_revision: 8,
            expected_position_revision: 0,
            observed_position_lamports: 11,
            observed_admission_lamports: 13,
            position_rent_principal: 11,
            admission_rent_principal: 13,
            capability_descriptor: [0; 32],
            capability_outcome: 0,
        }
    }

    fn close_request() -> ProtocolPositionRequestV2 {
        ProtocolPositionRequestV2 {
            action: ProtocolPositionActionV2::Close,
            presence: ProtocolPositionPresenceV2::Existing,
            expected_position_revision: 9,
            ..claims_request()
        }
    }

    #[test]
    fn exact_outer_round_trips_one_user_admission() {
        let request = UserPositionAdmissionRequestV1::new(claims_request()).expect("request");
        let bytes = request.to_bytes().expect("encode");
        assert_eq!(bytes.len(), 328);
        assert!(is_user_position_admission_v1(&bytes));
        assert_eq!(UserPositionAdmissionRequestV1::decode(&bytes), Ok(request));
        assert_eq!(request.claims_request_bytes().expect("child").len(), 320);
    }

    #[test]
    fn both_user_lifecycle_actions_round_trip_but_other_owners_refuse() {
        let mut hostile = claims_request();
        hostile.owner_kind = ProtocolPositionOwnerKindV2::TradingRecord;
        assert_eq!(
            UserPositionAdmissionRequestV1::new(hostile),
            Err(UserPositionAdmissionErrorV1::InvalidOwnerKind)
        );
        let close = UserPositionAdmissionRequestV1::new(close_request()).expect("close");
        let close_bytes = close.to_bytes().expect("close bytes");
        assert_eq!(
            UserPositionAdmissionRequestV1::decode(&close_bytes),
            Ok(close)
        );
        assert_eq!(
            close.validate_claims_receipt(
                &[0; PROTOCOL_POSITION_ADMISSION_BYTES_V2],
                [7; 32],
                [8; 32],
                [9; 32]
            ),
            Err(UserPositionAdmissionErrorV1::InvalidAction)
        );
        let mut bytes = UserPositionAdmissionRequestV1::new(claims_request())
            .expect("request")
            .to_bytes()
            .expect("bytes");
        bytes[13] = 1;
        assert_eq!(
            UserPositionAdmissionRequestV1::decode(&bytes),
            Err(UserPositionAdmissionErrorV1::InvalidClaimsRequest)
        );
        assert_eq!(
            UserPositionAdmissionRequestV1::decode(&bytes[..327]),
            Err(UserPositionAdmissionErrorV1::InvalidLength)
        );
    }

    #[test]
    fn outer_deescalates_the_pda_and_requires_only_the_wallet_signature() {
        let frame = UserPositionAdmissionFrameV1;
        assert_eq!(frame.account_count(), 27);
        for index in 0..frame.account_count() {
            frame.privileges(index).expect("coordinate");
        }
        assert!(
            !frame
                .privileges(USER_POSITION_ADMISSION_AUTHORITY_ACCOUNT_V1)
                .expect("authority")
                .signer()
        );
        assert!(
            frame
                .privileges(USER_POSITION_ADMISSION_OWNER_ACCOUNT_V1)
                .expect("owner")
                .signer()
        );
        assert!(
            frame
                .privileges(USER_POSITION_ADMISSION_CLAIMS_CALLEE_ACCOUNT_V1)
                .expect("callee")
                .executable()
        );
        assert!(
            frame
                .privileges(USER_POSITION_ADMISSION_CLAIMS_PROGRAM_ACCOUNT_V1)
                .expect("child claims")
                .executable()
        );
        assert_eq!(
            frame.privileges(frame.account_count()),
            Err(UserPositionAdmissionErrorV1::InvalidAccountCoordinate)
        );

        let close = UserPositionCloseFrameV1;
        assert_eq!(close.account_count(), 16);
        for index in 0..close.account_count() {
            close.privileges(index).expect("close coordinate");
        }
        assert!(
            !close
                .privileges(USER_POSITION_CLOSE_AUTHORITY_ACCOUNT_V1)
                .expect("close authority")
                .signer()
        );
        assert!(
            close
                .privileges(USER_POSITION_CLOSE_OWNER_ACCOUNT_V1)
                .expect("close owner")
                .signer()
        );
        assert!(
            close
                .privileges(USER_POSITION_CLOSE_CLAIMS_CALLEE_ACCOUNT_V1)
                .expect("close callee")
                .executable()
        );
        assert!(
            close
                .privileges(USER_POSITION_CLOSE_CLAIMS_PROGRAM_ACCOUNT_V1)
                .expect("close child claims")
                .executable()
        );
        assert_eq!(
            close.privileges(close.account_count()),
            Err(UserPositionAdmissionErrorV1::InvalidAccountCoordinate)
        );
    }
}
