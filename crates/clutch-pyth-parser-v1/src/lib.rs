#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]

//! Exact configuration and request wire contract for the first-party Pyth
//! parser used by SourceSeries 77/v2 action 4.
//!
//! The receiver posts a fully verified `PriceUpdateV2` before action 4. The
//! parser is a separate reviewed SBF release: it owns no feed state and only
//! validates the release-selected config, receiver-owned feed, Clock, boundary
//! crossing, freshness, and conservative integer normalization.

const CONFIG_MAGIC: [u8; 8] = *b"DCSPYP01";
const CONFIG_V2_MAGIC: [u8; 8] = *b"DCSPYP02";
const REQUEST_MAGIC: [u8; 8] = *b"DCSPYR01";
const VERSION: u16 = 1;

/// Exact immutable parser-config account-body width.
pub const PYTH_PARSER_CONFIG_BYTES: usize = 256;
/// Exact successor config width including receiver release and Config pins.
pub const PYTH_PARSER_CONFIG_V2_BYTES: usize = 512;
/// Exact parser instruction-data width constructed by Clutch.
pub const PYTH_PARSER_REQUEST_BYTES: usize = 24;

/// Fixed-codec refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input did not have the exact registered byte width.
    WrongLength,
    /// The fixed discriminator was not the registered discriminator.
    WrongMagic,
    /// The version was not exactly version one.
    BadVersion,
    /// A required public identity was all zero.
    ZeroIdentity,
    /// Two roles that must be distinct used the same identity.
    IdentityAlias,
    /// A normalization or freshness bound was outside the reviewed range.
    InvalidBound,
    /// Reserved bytes were not canonical zeroes.
    NonCanonicalPadding,
}

/// Immutable release-selected parser configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythParserConfigV1 {
    /// Physical identity of this configuration account.
    pub config_account: [u8; 32],
    /// Existing SourceSpec identity emitted in canonical parser output.
    pub source_spec_id: [u8; 32],
    /// Exact reviewed Pyth receiver program that must own the feed account.
    pub receiver_program: [u8; 32],
    /// Exact already-posted `PriceUpdateV2` account consumed by this route.
    pub feed_account: [u8; 32],
    /// Exact Pyth feed identifier inside `PriceUpdateV2`.
    pub pyth_feed_id: [u8; 32],
    /// Target decimal scale for Source V3 integer atoms.
    pub target_decimals: u8,
    /// Conservative confidence-radius multiplier.
    pub confidence_multiplier: u16,
    /// Maximum age of the Pyth publish time against canonical Clock.
    pub maximum_source_age_seconds: u64,
    /// Maximum lag of the receiver posted slot against canonical Clock.
    pub maximum_source_slot_lag: u64,
}

impl PythParserConfigV1 {
    /// Validate the complete closed configuration.
    pub fn validate(&self) -> Result<(), Error> {
        let identities = [
            self.config_account,
            self.source_spec_id,
            self.receiver_program,
            self.feed_account,
            self.pyth_feed_id,
        ];
        if identities.iter().any(|identity| *identity == [0; 32]) {
            return Err(Error::ZeroIdentity);
        }
        if self.config_account == self.receiver_program
            || self.config_account == self.feed_account
            || self.receiver_program == self.feed_account
        {
            return Err(Error::IdentityAlias);
        }
        if self.target_decimals > 18
            || self.confidence_multiplier == 0
            || self.maximum_source_age_seconds == 0
            || self.maximum_source_slot_lag == 0
        {
            return Err(Error::InvalidBound);
        }
        Ok(())
    }

    /// Encode the exact immutable account body.
    pub fn encode(&self) -> Result<[u8; PYTH_PARSER_CONFIG_BYTES], Error> {
        self.validate()?;
        let mut out = [0_u8; PYTH_PARSER_CONFIG_BYTES];
        out[..8].copy_from_slice(&CONFIG_MAGIC);
        out[8..10].copy_from_slice(&VERSION.to_le_bytes());
        out[16..48].copy_from_slice(&self.config_account);
        out[48..80].copy_from_slice(&self.source_spec_id);
        out[80..112].copy_from_slice(&self.receiver_program);
        out[112..144].copy_from_slice(&self.feed_account);
        out[144..176].copy_from_slice(&self.pyth_feed_id);
        out[176] = self.target_decimals;
        out[178..180].copy_from_slice(&self.confidence_multiplier.to_le_bytes());
        out[184..192].copy_from_slice(&self.maximum_source_age_seconds.to_le_bytes());
        out[192..200].copy_from_slice(&self.maximum_source_slot_lag.to_le_bytes());
        Ok(out)
    }

    /// Decode an exact, canonical immutable account body.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != PYTH_PARSER_CONFIG_BYTES {
            return Err(Error::WrongLength);
        }
        if input[..8] != CONFIG_MAGIC {
            return Err(Error::WrongMagic);
        }
        if u16_at(input, 8) != VERSION {
            return Err(Error::BadVersion);
        }
        if input[10..16].iter().any(|byte| *byte != 0)
            || input[177] != 0
            || input[180..184].iter().any(|byte| *byte != 0)
            || input[200..].iter().any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalPadding);
        }
        let value = Self {
            config_account: array_32(input, 16),
            source_spec_id: array_32(input, 48),
            receiver_program: array_32(input, 80),
            feed_account: array_32(input, 112),
            pyth_feed_id: array_32(input, 144),
            target_decimals: input[176],
            confidence_multiplier: u16_at(input, 178),
            maximum_source_age_seconds: u64_at(input, 184),
            maximum_source_slot_lag: u64_at(input, 192),
        };
        value.validate()?;
        Ok(value)
    }
}

/// Successor parser config pinning the receiver ProgramData release and its
/// complete mutable Config bytes without reinterpreting version one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythParserConfigV2 {
    /// Complete historical v1 route body.
    pub base: PythParserConfigV1,
    /// Receiver ProgramData account linked by the receiver Program.
    pub receiver_programdata: [u8; 32],
    /// Digest of complete receiver Program account bytes.
    pub receiver_program_account_data_id: [u8; 32],
    /// Digest of complete receiver ProgramData bytes, including its ELF.
    pub receiver_programdata_account_data_id: [u8; 32],
    /// Exact upgradeable loader owning Program and ProgramData.
    pub receiver_loader: [u8; 32],
    /// Offset of the ProgramData key in the receiver Program account.
    pub receiver_programdata_link_offset: u16,
    /// Offset of the deployment slot in receiver ProgramData.
    pub receiver_deployment_slot_offset: u16,
    /// Exact receiver deployment slot.
    pub receiver_deployment_slot: u64,
    /// Exact receiver Config account.
    pub receiver_config: [u8; 32],
    /// Exact receiver Config owner; must equal the receiver Program.
    pub receiver_config_owner: [u8; 32],
    /// Digest of complete receiver Config account bytes.
    pub receiver_config_data_id: [u8; 32],
}

impl PythParserConfigV2 {
    /// Validate the complete parser and receiver release route.
    pub fn validate(&self) -> Result<(), Error> {
        self.base.validate()?;
        for identity in [
            self.receiver_programdata,
            self.receiver_program_account_data_id,
            self.receiver_programdata_account_data_id,
            self.receiver_loader,
            self.receiver_config,
            self.receiver_config_owner,
            self.receiver_config_data_id,
        ] {
            if identity == [0; 32] {
                return Err(Error::ZeroIdentity);
            }
        }
        if self.receiver_deployment_slot == 0 {
            return Err(Error::InvalidBound);
        }
        if self.receiver_config_owner != self.base.receiver_program
            || self.receiver_programdata == self.base.receiver_program
            || self.receiver_loader == self.base.receiver_program
            || self.receiver_loader == self.receiver_programdata
            || self.receiver_config == self.base.receiver_program
            || self.receiver_config == self.receiver_programdata
            || self.receiver_config == self.base.feed_account
            || usize::from(self.receiver_programdata_link_offset)
                .checked_add(32)
                .is_none()
            || usize::from(self.receiver_deployment_slot_offset)
                .checked_add(8)
                .is_none()
        {
            return Err(Error::IdentityAlias);
        }
        Ok(())
    }

    /// Encode the exact successor parser Config body.
    pub fn encode(&self) -> Result<[u8; PYTH_PARSER_CONFIG_V2_BYTES], Error> {
        self.validate()?;
        let mut out = [0_u8; PYTH_PARSER_CONFIG_V2_BYTES];
        out[..8].copy_from_slice(&CONFIG_V2_MAGIC);
        out[8..10].copy_from_slice(&2_u16.to_le_bytes());
        out[16..272].copy_from_slice(&self.base.encode()?);
        out[272..304].copy_from_slice(&self.receiver_programdata);
        out[304..336].copy_from_slice(&self.receiver_program_account_data_id);
        out[336..368].copy_from_slice(&self.receiver_programdata_account_data_id);
        out[368..400].copy_from_slice(&self.receiver_loader);
        out[400..402].copy_from_slice(&self.receiver_programdata_link_offset.to_le_bytes());
        out[402..404].copy_from_slice(&self.receiver_deployment_slot_offset.to_le_bytes());
        out[408..416].copy_from_slice(&self.receiver_deployment_slot.to_le_bytes());
        out[416..448].copy_from_slice(&self.receiver_config);
        out[448..480].copy_from_slice(&self.receiver_config_owner);
        out[480..512].copy_from_slice(&self.receiver_config_data_id);
        Ok(out)
    }

    /// Decode exact successor bytes; v1 never auto-upgrades.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != PYTH_PARSER_CONFIG_V2_BYTES {
            return Err(Error::WrongLength);
        }
        if input[..8] != CONFIG_V2_MAGIC {
            return Err(Error::WrongMagic);
        }
        if u16_at(input, 8) != 2 {
            return Err(Error::BadVersion);
        }
        if input[10..16].iter().any(|byte| *byte != 0)
            || input[404..408].iter().any(|byte| *byte != 0)
        {
            return Err(Error::NonCanonicalPadding);
        }
        let value = Self {
            base: PythParserConfigV1::decode(&input[16..272])?,
            receiver_programdata: array_32(input, 272),
            receiver_program_account_data_id: array_32(input, 304),
            receiver_programdata_account_data_id: array_32(input, 336),
            receiver_loader: array_32(input, 368),
            receiver_programdata_link_offset: u16_at(input, 400),
            receiver_deployment_slot_offset: u16_at(input, 402),
            receiver_deployment_slot: u64_at(input, 408),
            receiver_config: array_32(input, 416),
            receiver_config_owner: array_32(input, 448),
            receiver_config_data_id: array_32(input, 480),
        };
        value.validate()?;
        Ok(value)
    }
}

/// State-derived request sent by Clutch to the reviewed parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PythParserRequestV1 {
    /// Exact wall-clock boundary derived from release policy and OpenRawPage.
    pub boundary_unix_seconds: u64,
}

impl PythParserRequestV1 {
    /// Validate that the boundary is a live timestamp.
    pub fn validate(&self) -> Result<(), Error> {
        if self.boundary_unix_seconds == 0 {
            return Err(Error::InvalidBound);
        }
        Ok(())
    }

    /// Encode the exact CPI instruction data.
    pub fn encode(&self) -> Result<[u8; PYTH_PARSER_REQUEST_BYTES], Error> {
        self.validate()?;
        let mut out = [0_u8; PYTH_PARSER_REQUEST_BYTES];
        out[..8].copy_from_slice(&REQUEST_MAGIC);
        out[8..10].copy_from_slice(&VERSION.to_le_bytes());
        out[16..24].copy_from_slice(&self.boundary_unix_seconds.to_le_bytes());
        Ok(out)
    }

    /// Decode exact CPI instruction data.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != PYTH_PARSER_REQUEST_BYTES {
            return Err(Error::WrongLength);
        }
        if input[..8] != REQUEST_MAGIC {
            return Err(Error::WrongMagic);
        }
        if u16_at(input, 8) != VERSION {
            return Err(Error::BadVersion);
        }
        if input[10..16].iter().any(|byte| *byte != 0) {
            return Err(Error::NonCanonicalPadding);
        }
        let value = Self {
            boundary_unix_seconds: u64_at(input, 16),
        };
        value.validate()?;
        Ok(value)
    }
}

fn array_32(input: &[u8], at: usize) -> [u8; 32] {
    let mut out = [0_u8; 32];
    out.copy_from_slice(&input[at..at + 32]);
    out
}

fn u16_at(input: &[u8], at: usize) -> u16 {
    let mut bytes = [0_u8; 2];
    bytes.copy_from_slice(&input[at..at + 2]);
    u16::from_le_bytes(bytes)
}

fn u64_at(input: &[u8], at: usize) -> u64 {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&input[at..at + 8]);
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> PythParserConfigV1 {
        PythParserConfigV1 {
            config_account: [1; 32],
            source_spec_id: [2; 32],
            receiver_program: [3; 32],
            feed_account: [4; 32],
            pyth_feed_id: [5; 32],
            target_decimals: 8,
            confidence_multiplier: 2,
            maximum_source_age_seconds: 30,
            maximum_source_slot_lag: 150,
        }
    }

    fn config_v2() -> PythParserConfigV2 {
        PythParserConfigV2 {
            base: config(),
            receiver_programdata: [6; 32],
            receiver_program_account_data_id: [7; 32],
            receiver_programdata_account_data_id: [8; 32],
            receiver_loader: [9; 32],
            receiver_programdata_link_offset: 4,
            receiver_deployment_slot_offset: 4,
            receiver_deployment_slot: 100,
            receiver_config: [10; 32],
            receiver_config_owner: [3; 32],
            receiver_config_data_id: [11; 32],
        }
    }

    #[test]
    fn config_round_trips_exactly() {
        let bytes = config().encode().unwrap();
        assert_eq!(bytes.len(), PYTH_PARSER_CONFIG_BYTES);
        assert_eq!(PythParserConfigV1::decode(&bytes), Ok(config()));
    }

    #[test]
    fn dirty_config_padding_refuses() {
        let mut bytes = config().encode().unwrap();
        bytes[255] = 1;
        assert_eq!(
            PythParserConfigV1::decode(&bytes),
            Err(Error::NonCanonicalPadding)
        );
    }

    #[test]
    fn aliased_receiver_and_feed_refuses() {
        let mut value = config();
        value.feed_account = value.receiver_program;
        assert_eq!(value.encode(), Err(Error::IdentityAlias));
    }

    #[test]
    fn zero_confidence_multiplier_refuses() {
        let mut value = config();
        value.confidence_multiplier = 0;
        assert_eq!(value.encode(), Err(Error::InvalidBound));
    }

    #[test]
    fn request_round_trips_and_rejects_padding() {
        let request = PythParserRequestV1 {
            boundary_unix_seconds: 1_700_000_000,
        };
        let mut bytes = request.encode().unwrap();
        assert_eq!(PythParserRequestV1::decode(&bytes), Ok(request));
        bytes[10] = 1;
        assert_eq!(
            PythParserRequestV1::decode(&bytes),
            Err(Error::NonCanonicalPadding)
        );
    }

    #[test]
    fn successor_config_round_trips_and_v1_never_auto_upgrades() {
        let bytes = config_v2().encode().unwrap();
        assert_eq!(PythParserConfigV2::decode(&bytes), Ok(config_v2()));
        assert_eq!(
            PythParserConfigV2::decode(&config().encode().unwrap()),
            Err(Error::WrongLength)
        );
    }

    #[test]
    fn dirty_successor_padding_and_wrong_config_owner_refuse() {
        let mut bytes = config_v2().encode().unwrap();
        bytes[405] = 1;
        assert_eq!(
            PythParserConfigV2::decode(&bytes),
            Err(Error::NonCanonicalPadding)
        );
        let mut value = config_v2();
        value.receiver_config_owner = [12; 32];
        assert_eq!(value.encode(), Err(Error::IdentityAlias));
    }
}
