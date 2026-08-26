//! Exact real-provider execution request and receipt for the successor route.
//!
//! The request distinguishes the provider submitter retained by Pyth's update,
//! the permissionless resolver executing Source, and the current Core or
//! Trading caller. A Trading caller additionally binds the exact
//! CapabilityProgramSet and selected CapabilityProgram; a Core caller must not
//! smuggle those coordinates into its fixed-role route.

use crate::{Error, Result};

/// Exact real-provider request width.
pub const PROVIDER_EXECUTION_REQUEST_BYTES_V3: usize = 608;
/// Exact real-provider receipt width.
pub const PROVIDER_EXECUTION_RECEIPT_BYTES_V3: usize = 672;
/// Request magic.
pub const PROVIDER_EXECUTION_REQUEST_MAGIC_V3: [u8; 8] = *b"DCLTPRQ3";
/// Receipt magic.
pub const PROVIDER_EXECUTION_RECEIPT_MAGIC_V3: [u8; 8] = *b"DCLTPRC3";
/// Shared schema version.
pub const PROVIDER_EXECUTION_VERSION_V3: u16 = 3;
/// Consume one already provider-authenticated Pyth update.
pub const PROVIDER_EXECUTION_ACTION_V3: u8 = 1;
/// Canonical schema label for the fixed request prefix.
pub const PROVIDER_EXECUTION_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] =
    b"dclutch/schema/provider-resolution-request-v3";
/// SHA-256 of [`PROVIDER_EXECUTION_REQUEST_SCHEMA_PREIMAGE_V3`].
pub const PROVIDER_EXECUTION_REQUEST_SCHEMA_ID_V3: [u8; 32] = [
    0xcd, 0x32, 0xad, 0xc8, 0x75, 0xf9, 0x18, 0x1e, 0xe9, 0x64, 0x3b, 0x08, 0x05, 0xd3, 0xe4, 0x1f,
    0x79, 0x03, 0x7d, 0x9d, 0xbc, 0xf3, 0xc7, 0x70, 0xde, 0x42, 0x35, 0xb6, 0x40, 0x78, 0xd2, 0xde,
];

/// Exact account count for the fixed Core caller frame.
pub const PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3: usize = 46;
/// Exact account count for the Trading caller frame, including its selected
/// ProgramSet and descriptor raw/staging pairs.
pub const PROVIDER_RESOLUTION_TRADING_ACCOUNT_COUNT_V3: usize = 50;
/// First common account index. The order from this index is caller authority,
/// resolver, Source state, certificate, Market, activation, infrastructure
/// profile, Registry Program/ProgramData/artifact pair, Core Program/ProgramData,
/// Trading Program/ProgramData, and Resolution Program/ProgramData.
pub const PROVIDER_RESOLUTION_CALLER_AUTHORITY_ACCOUNT_V3: usize = 0;
/// Permissionless resolver signer.
pub const PROVIDER_RESOLUTION_RESOLVER_ACCOUNT_V3: usize = 1;
/// Writable Source state.
pub const PROVIDER_RESOLUTION_SOURCE_STATE_ACCOUNT_V3: usize = 2;
/// Writable deterministic certificate destination.
pub const PROVIDER_RESOLUTION_CERTIFICATE_ACCOUNT_V3: usize = 3;
/// Core Market account.
pub const PROVIDER_RESOLUTION_MARKET_ACCOUNT_V3: usize = 4;
/// Registry-owned activated execution release set.
pub const PROVIDER_RESOLUTION_ACTIVATION_ACCOUNT_V3: usize = 5;
/// Core-owned immutable Registry/Rent infrastructure selection.
pub const PROVIDER_RESOLUTION_INFRASTRUCTURE_ACCOUNT_V3: usize = 6;
/// First Registry deployment account: Registry Program, ProgramData, artifact
/// raw, and vacant artifact staging cursor.
pub const PROVIDER_RESOLUTION_REGISTRY_ACCOUNTS_START_V3: usize = 7;
/// First current role deployment account: Core, Trading, and Resolution
/// Program/ProgramData pairs in that order.
pub const PROVIDER_RESOLUTION_ROLE_ACCOUNTS_START_V3: usize = 11;
/// First Source finalized-record account. Seven raw/staging pairs follow in
/// order: material, SourceSpec, ProviderRelease, PythAdapterConfig, WindowSpec,
/// StatisticSpec, and PythRelease.
pub const PROVIDER_RESOLUTION_SOURCE_RECORDS_START_V3: usize = 17;
/// First Product Runtime V2 account: Product, result-domain, and portfolio
/// raw/staging pairs.
pub const PROVIDER_RESOLUTION_PRODUCT_RECORDS_START_V3: usize = 31;
/// First Trading-only account: ProgramSet and selected CapabilityProgramV3
/// raw/staging pairs.
pub const PROVIDER_RESOLUTION_TRADING_RECORDS_START_V3: usize = 37;
/// Core tail start. The tail is update, Receiver Program/ProgramData/config,
/// router Program/ProgramData, Clock, Rent, and System Program.
pub const PROVIDER_RESOLUTION_CORE_TAIL_START_V3: usize = 37;
/// Trading tail start after its four additional finalized-record accounts.
pub const PROVIDER_RESOLUTION_TRADING_TAIL_START_V3: usize = 41;

const GENERATION_OFFSET: usize = 16;
const TERMINAL_SEQUENCE_OFFSET: usize = 24;
const REQUEST_IDENTITIES_OFFSET: usize = 32;
const REQUEST_IDENTITY_COUNT: usize = 18;
const RECEIPT_IDENTITIES_OFFSET: usize = 32;
const RECEIPT_IDENTITY_COUNT: usize = 18;

/// Authenticated protocol caller of the Resolution child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProviderCallerV3 {
    /// Current Registry-selected Core fixed Resolution role.
    Core = 1,
    /// Current Registry-selected Trading V3 interpreter route.
    Trading = 2,
}

impl ProviderCallerV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Core),
            2 => Ok(Self::Trading),
            _ => Err(Error::UnknownAction),
        }
    }
}

/// Exact optimistic coordinates for a real-provider terminal resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderExecutionRequestV3 {
    /// Typed protocol caller.
    pub caller: ProviderCallerV3,
    /// Immutable Market generation.
    pub generation: u64,
    /// Positive terminal replay sequence.
    pub terminal_sequence: u64,
    /// Core Market account.
    pub market: [u8; 32],
    /// Runtime-width Source state.
    pub source_state: [u8; 32],
    /// Deterministic typed terminal certificate destination.
    pub certificate_account: [u8; 32],
    /// Exact SourceMaterialV2 content identity.
    pub source_material: [u8; 32],
    /// Exact primary SourceSpec content identity.
    pub source_spec: [u8; 32],
    /// Product Runtime V2 root content identity.
    pub product_record: [u8; 32],
    /// Product-owned result-domain content identity.
    pub result_domain: [u8; 32],
    /// Exact Pyth deployment-release record identity selected by Source.
    pub provider_release: [u8; 32],
    /// Provider-owned Pyth update account.
    pub update_account: [u8; 32],
    /// SHA-256 of the exact provider-owned update bytes.
    pub expected_update_digest: [u8; 32],
    /// Submitter retained as the Pyth update's write authority.
    pub provider_submitter: [u8; 32],
    /// Permissionless resolver executing this transition.
    pub resolver: [u8; 32],
    /// Current Registry-selected Core or Trading program.
    pub caller_program: [u8; 32],
    /// Market-selected activated release set.
    pub release_set: [u8; 32],
    /// Trading CapabilityProgramSet content identity, or zero for Core.
    pub capability_program_set: [u8; 32],
    /// Trading action-selected CapabilityProgramV3 identity, or zero for Core.
    pub selected_capability_program: [u8; 32],
    /// Exact parent Core-effect or Trading-family request digest.
    pub parent_request_digest: [u8; 32],
    /// SHA-256 of the exact Pyth PostUpdateParams body.
    pub post_params_body_digest: [u8; 32],
}

impl ProviderExecutionRequestV3 {
    /// Decode one exact canonical request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PROVIDER_EXECUTION_REQUEST_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != PROVIDER_EXECUTION_REQUEST_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != PROVIDER_EXECUTION_VERSION_V3
            || byte(bytes, 10)? != PROVIDER_EXECUTION_ACTION_V3
        {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(bytes, 12, 4)?;
        let mut identities = [[0_u8; 32]; REQUEST_IDENTITY_COUNT];
        for (index, identity) in identities.iter_mut().enumerate() {
            *identity = array(
                bytes,
                REQUEST_IDENTITIES_OFFSET
                    .checked_add(index.checked_mul(32).ok_or(Error::InvalidLength)?)
                    .ok_or(Error::InvalidLength)?,
            )?;
        }
        let value = Self {
            caller: ProviderCallerV3::decode(byte(bytes, 11)?)?,
            generation: read_u64(bytes, GENERATION_OFFSET)?,
            terminal_sequence: read_u64(bytes, TERMINAL_SEQUENCE_OFFSET)?,
            market: identities[0],
            source_state: identities[1],
            certificate_account: identities[2],
            source_material: identities[3],
            source_spec: identities[4],
            product_record: identities[5],
            result_domain: identities[6],
            provider_release: identities[7],
            update_account: identities[8],
            expected_update_digest: identities[9],
            provider_submitter: identities[10],
            resolver: identities[11],
            caller_program: identities[12],
            release_set: identities[13],
            capability_program_set: identities[14],
            selected_capability_program: identities[15],
            parent_request_digest: identities[16],
            post_params_body_digest: identities[17],
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical request.
    pub fn to_bytes(self) -> Result<[u8; PROVIDER_EXECUTION_REQUEST_BYTES_V3]> {
        self.validate()?;
        let mut bytes = [0_u8; PROVIDER_EXECUTION_REQUEST_BYTES_V3];
        bytes[..8].copy_from_slice(&PROVIDER_EXECUTION_REQUEST_MAGIC_V3);
        bytes[8..10].copy_from_slice(&PROVIDER_EXECUTION_VERSION_V3.to_le_bytes());
        bytes[10] = PROVIDER_EXECUTION_ACTION_V3;
        bytes[11] = self.caller as u8;
        bytes[GENERATION_OFFSET..GENERATION_OFFSET + 8]
            .copy_from_slice(&self.generation.to_le_bytes());
        bytes[TERMINAL_SEQUENCE_OFFSET..TERMINAL_SEQUENCE_OFFSET + 8]
            .copy_from_slice(&self.terminal_sequence.to_le_bytes());
        for (index, identity) in self.identities().iter().enumerate() {
            let offset = REQUEST_IDENTITIES_OFFSET + index * 32;
            put(&mut bytes, offset, identity)?;
        }
        Ok(bytes)
    }

    fn identities(self) -> [[u8; 32]; REQUEST_IDENTITY_COUNT] {
        [
            self.market,
            self.source_state,
            self.certificate_account,
            self.source_material,
            self.source_spec,
            self.product_record,
            self.result_domain,
            self.provider_release,
            self.update_account,
            self.expected_update_digest,
            self.provider_submitter,
            self.resolver,
            self.caller_program,
            self.release_set,
            self.capability_program_set,
            self.selected_capability_program,
            self.parent_request_digest,
            self.post_params_body_digest,
        ]
    }

    fn validate(self) -> Result<()> {
        if self.generation == 0
            || self.terminal_sequence == 0
            || self
                .identities()
                .iter()
                .take(14)
                .chain(self.identities().iter().skip(16))
                .any(is_zero)
            || self.provider_submitter == self.resolver
            || self.provider_submitter == self.caller_program
            || self.provider_submitter == self.update_account
            || self.resolver == self.caller_program
            || self.resolver == self.update_account
            || self.caller_program == self.update_account
        {
            return Err(Error::ZeroCoordinate);
        }
        match self.caller {
            ProviderCallerV3::Core
                if !is_zero(&self.capability_program_set)
                    || !is_zero(&self.selected_capability_program) =>
            {
                Err(Error::InvalidReceiptShape)
            }
            ProviderCallerV3::Trading
                if is_zero(&self.capability_program_set)
                    || is_zero(&self.selected_capability_program) =>
            {
                Err(Error::InvalidReceiptShape)
            }
            _ => Ok(()),
        }
    }
}

/// Caller-verifiable receipt for one exact provider-to-Product join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderExecutionReceiptV3 {
    /// Typed protocol caller.
    pub caller: ProviderCallerV3,
    /// Immutable Market generation.
    pub generation: u64,
    /// Positive terminal replay sequence.
    pub terminal_sequence: u64,
    /// SHA-256 of the complete request bytes.
    pub request_digest: [u8; 32],
    /// Domain-separated provider evidence identity.
    pub provider_evidence: [u8; 32],
    /// SHA-256 of exact provider-owned update bytes.
    pub update_digest: [u8; 32],
    /// SHA-256 of exact PostUpdateParams bytes.
    pub post_params_body_digest: [u8; 32],
    /// Core Market account.
    pub market: [u8; 32],
    /// Source state account.
    pub source_state: [u8; 32],
    /// Deterministic typed terminal certificate account.
    pub certificate_account: [u8; 32],
    /// Source material identity.
    pub source_material: [u8; 32],
    /// Product Runtime V2 root identity.
    pub product_record: [u8; 32],
    /// Product-owned result-domain identity.
    pub result_domain: [u8; 32],
    /// Pyth deployment-release record identity.
    pub provider_release: [u8; 32],
    /// Provider-owned update account.
    pub update_account: [u8; 32],
    /// Provider submitter derived from Pyth update bytes.
    pub provider_submitter: [u8; 32],
    /// Permissionless resolver.
    pub resolver: [u8; 32],
    /// Current Core or Trading program.
    pub caller_program: [u8; 32],
    /// Market-selected activated release set.
    pub release_set: [u8; 32],
    /// Trading CapabilityProgramSet identity, or zero for Core.
    pub capability_program_set: [u8; 32],
    /// Trading selected CapabilityProgram identity, or zero for Core.
    pub selected_capability_program: [u8; 32],
    /// Ordinary selector derived from Product's exact result domain.
    pub selector: u32,
    /// Product native outcome count including explicit failure.
    pub outcome_count: u32,
    /// Exact normalized result numerator.
    pub result_numerator: i128,
    /// Positive exact normalized result denominator.
    pub result_denominator: u64,
    /// Provider publication time.
    pub publish_time: i64,
    /// Provider-posted slot retained by Pyth.
    pub posted_slot: u64,
    /// Slot at which the update was consumed.
    pub consumed_slot: u64,
}

impl ProviderExecutionReceiptV3 {
    /// Decode one exact canonical receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PROVIDER_EXECUTION_RECEIPT_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != PROVIDER_EXECUTION_RECEIPT_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != PROVIDER_EXECUTION_VERSION_V3
            || byte(bytes, 10)? != PROVIDER_EXECUTION_ACTION_V3
        {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(bytes, 12, 4)?;
        require_zero(bytes, 664, 8)?;
        let mut identities = [[0_u8; 32]; RECEIPT_IDENTITY_COUNT];
        for (index, identity) in identities.iter_mut().enumerate() {
            *identity = array(bytes, RECEIPT_IDENTITIES_OFFSET + index * 32)?;
        }
        let value = Self {
            caller: ProviderCallerV3::decode(byte(bytes, 11)?)?,
            generation: read_u64(bytes, GENERATION_OFFSET)?,
            terminal_sequence: read_u64(bytes, TERMINAL_SEQUENCE_OFFSET)?,
            request_digest: identities[0],
            provider_evidence: identities[1],
            update_digest: identities[2],
            post_params_body_digest: identities[3],
            market: identities[4],
            source_state: identities[5],
            certificate_account: identities[6],
            source_material: identities[7],
            product_record: identities[8],
            result_domain: identities[9],
            provider_release: identities[10],
            update_account: identities[11],
            provider_submitter: identities[12],
            resolver: identities[13],
            caller_program: identities[14],
            release_set: identities[15],
            capability_program_set: identities[16],
            selected_capability_program: identities[17],
            selector: read_u32(bytes, 608)?,
            outcome_count: read_u32(bytes, 612)?,
            result_numerator: read_i128(bytes, 616)?,
            result_denominator: read_u64(bytes, 632)?,
            publish_time: read_i64(bytes, 640)?,
            posted_slot: read_u64(bytes, 648)?,
            consumed_slot: read_u64(bytes, 656)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode one exact canonical receipt.
    pub fn to_bytes(self) -> Result<[u8; PROVIDER_EXECUTION_RECEIPT_BYTES_V3]> {
        self.validate()?;
        let mut bytes = [0_u8; PROVIDER_EXECUTION_RECEIPT_BYTES_V3];
        bytes[..8].copy_from_slice(&PROVIDER_EXECUTION_RECEIPT_MAGIC_V3);
        bytes[8..10].copy_from_slice(&PROVIDER_EXECUTION_VERSION_V3.to_le_bytes());
        bytes[10] = PROVIDER_EXECUTION_ACTION_V3;
        bytes[11] = self.caller as u8;
        bytes[GENERATION_OFFSET..GENERATION_OFFSET + 8]
            .copy_from_slice(&self.generation.to_le_bytes());
        bytes[TERMINAL_SEQUENCE_OFFSET..TERMINAL_SEQUENCE_OFFSET + 8]
            .copy_from_slice(&self.terminal_sequence.to_le_bytes());
        for (index, identity) in self.identities().iter().enumerate() {
            let offset = RECEIPT_IDENTITIES_OFFSET + index * 32;
            put(&mut bytes, offset, identity)?;
        }
        bytes[608..612].copy_from_slice(&self.selector.to_le_bytes());
        bytes[612..616].copy_from_slice(&self.outcome_count.to_le_bytes());
        bytes[616..632].copy_from_slice(&self.result_numerator.to_le_bytes());
        bytes[632..640].copy_from_slice(&self.result_denominator.to_le_bytes());
        bytes[640..648].copy_from_slice(&self.publish_time.to_le_bytes());
        bytes[648..656].copy_from_slice(&self.posted_slot.to_le_bytes());
        bytes[656..664].copy_from_slice(&self.consumed_slot.to_le_bytes());
        Ok(bytes)
    }

    fn identities(self) -> [[u8; 32]; RECEIPT_IDENTITY_COUNT] {
        [
            self.request_digest,
            self.provider_evidence,
            self.update_digest,
            self.post_params_body_digest,
            self.market,
            self.source_state,
            self.certificate_account,
            self.source_material,
            self.product_record,
            self.result_domain,
            self.provider_release,
            self.update_account,
            self.provider_submitter,
            self.resolver,
            self.caller_program,
            self.release_set,
            self.capability_program_set,
            self.selected_capability_program,
        ]
    }

    fn validate(self) -> Result<()> {
        let failure_selector = self
            .outcome_count
            .checked_sub(1)
            .filter(|_| self.outcome_count >= 2)
            .ok_or(Error::InvalidSelector)?;
        if self.generation == 0
            || self.terminal_sequence == 0
            || self.result_denominator == 0
            || self.publish_time <= 0
            || self.consumed_slot < self.posted_slot
            || self.selector >= failure_selector
            || self.identities().iter().take(16).any(is_zero)
            || self.provider_submitter == self.resolver
            || self.provider_submitter == self.caller_program
            || self.provider_submitter == self.update_account
            || self.resolver == self.caller_program
            || self.resolver == self.update_account
            || self.caller_program == self.update_account
        {
            return Err(Error::InvalidReceiptShape);
        }
        match self.caller {
            ProviderCallerV3::Core
                if !is_zero(&self.capability_program_set)
                    || !is_zero(&self.selected_capability_program) =>
            {
                Err(Error::InvalidReceiptShape)
            }
            ProviderCallerV3::Trading
                if is_zero(&self.capability_program_set)
                    || is_zero(&self.selected_capability_program) =>
            {
                Err(Error::InvalidReceiptShape)
            }
            _ => Ok(()),
        }
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn read_i64(bytes: &[u8], offset: usize) -> Result<i64> {
    Ok(i64::from_le_bytes(array(bytes, offset)?))
}

fn read_i128(bytes: &[u8], offset: usize) -> Result<i128> {
    Ok(i128::from_le_bytes(array(bytes, offset)?))
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    if bytes
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .all(|value| *value == 0)
    {
        Ok(())
    } else {
        Err(Error::NonCanonicalReserved)
    }
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

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn request(caller: ProviderCallerV3) -> ProviderExecutionRequestV3 {
        ProviderExecutionRequestV3 {
            caller,
            generation: 7,
            terminal_sequence: 11,
            market: identity(1),
            source_state: identity(2),
            certificate_account: identity(3),
            source_material: identity(4),
            source_spec: identity(5),
            product_record: identity(6),
            result_domain: identity(7),
            provider_release: identity(8),
            update_account: identity(9),
            expected_update_digest: identity(10),
            provider_submitter: identity(11),
            resolver: identity(12),
            caller_program: identity(13),
            release_set: identity(14),
            capability_program_set: if caller == ProviderCallerV3::Trading {
                identity(15)
            } else {
                [0; 32]
            },
            selected_capability_program: if caller == ProviderCallerV3::Trading {
                identity(16)
            } else {
                [0; 32]
            },
            parent_request_digest: identity(17),
            post_params_body_digest: identity(18),
        }
    }

    fn receipt(caller: ProviderCallerV3) -> ProviderExecutionReceiptV3 {
        let request = request(caller);
        ProviderExecutionReceiptV3 {
            caller,
            generation: request.generation,
            terminal_sequence: request.terminal_sequence,
            request_digest: identity(20),
            provider_evidence: identity(21),
            update_digest: request.expected_update_digest,
            post_params_body_digest: request.post_params_body_digest,
            market: request.market,
            source_state: request.source_state,
            certificate_account: request.certificate_account,
            source_material: request.source_material,
            product_record: request.product_record,
            result_domain: request.result_domain,
            provider_release: request.provider_release,
            update_account: request.update_account,
            provider_submitter: request.provider_submitter,
            resolver: request.resolver,
            caller_program: request.caller_program,
            release_set: request.release_set,
            capability_program_set: request.capability_program_set,
            selected_capability_program: request.selected_capability_program,
            selector: 4,
            outcome_count: 258,
            result_numerator: -123,
            result_denominator: 7,
            publish_time: 1_700_000_000,
            posted_slot: 30,
            consumed_slot: 31,
        }
    }

    #[test]
    fn both_current_caller_profiles_round_trip_exactly() {
        for caller in [ProviderCallerV3::Core, ProviderCallerV3::Trading] {
            let request = request(caller);
            let request_bytes = request.to_bytes().expect("canonical request");
            assert_eq!(
                ProviderExecutionRequestV3::decode(&request_bytes),
                Ok(request)
            );
            let receipt = receipt(caller);
            let receipt_bytes = receipt.to_bytes().expect("canonical receipt");
            assert_eq!(
                ProviderExecutionReceiptV3::decode(&receipt_bytes),
                Ok(receipt)
            );
        }
    }

    #[test]
    fn caller_authority_shapes_cannot_cross() {
        let mut core = request(ProviderCallerV3::Core);
        core.capability_program_set = identity(90);
        assert_eq!(core.to_bytes(), Err(Error::InvalidReceiptShape));
        let mut trading = request(ProviderCallerV3::Trading);
        trading.selected_capability_program = [0; 32];
        assert_eq!(trading.to_bytes(), Err(Error::InvalidReceiptShape));
    }

    #[test]
    fn role_aliases_and_explicit_failure_selection_refuse() {
        let mut aliased = request(ProviderCallerV3::Core);
        aliased.resolver = aliased.provider_submitter;
        assert!(aliased.to_bytes().is_err());
        let mut failure = receipt(ProviderCallerV3::Trading);
        failure.selector = failure.outcome_count - 1;
        assert_eq!(failure.to_bytes(), Err(Error::InvalidReceiptShape));
    }

    #[test]
    fn hostile_headers_reserved_bytes_and_widths_refuse() {
        let bytes = request(ProviderCallerV3::Core)
            .to_bytes()
            .expect("canonical request");
        for length in [0, 8, PROVIDER_EXECUTION_REQUEST_BYTES_V3 - 1] {
            assert_eq!(
                ProviderExecutionRequestV3::decode(
                    bytes.get(..length).expect("hostile test prefix")
                ),
                Err(Error::InvalidLength)
            );
        }
        let mut hostile = bytes;
        hostile[12] = 1;
        assert_eq!(
            ProviderExecutionRequestV3::decode(&hostile),
            Err(Error::NonCanonicalReserved)
        );
    }
}
