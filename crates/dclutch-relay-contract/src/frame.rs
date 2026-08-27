//! Exact ordered account frames for the four relay record routes.
//!
//! Counts, privileges and the complete no-alias policy are checked here; the
//! *semantic* identity of each position (that this really is the Market, that
//! this really is the pinned raw record) stays an adapter obligation and is
//! authenticated separately by PDA derivation and content identity.
//!
//! These frames are relay-owned rather than added to `dclutch-source-contract`.
//! The relay routes are their own instruction family with their own magic, they
//! name account classes that no Source route has (a relayer key set, an
//! observation record), and `SourceAccountRoleV1`'s constructor is private to
//! its crate.  The Source acceptance frame that *consumes* a sealed record is a
//! separate question and belongs to `dclutch-source-contract`.

use crate::{ADDRESS_BYTES, Error, Result};

/// Semantic role name in one ordered relay frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayAccountNameV1 {
    /// The permissionless worker paying for and signing the transaction.
    Worker,
    /// The owning Market root.
    Market,
    /// The observation record being created, appended to, sealed or retired.
    Record,
    /// The raw immutable `SourceMaterialV2` record.
    SourceMaterial,
    /// The finalized staging vacancy proving the material record is immutable.
    SourceMaterialStagingVacancy,
    /// The raw immutable `RelayerKeySetV1` record.
    RelayerKeySet,
    /// The finalized staging vacancy proving the key set is immutable.
    RelayerKeySetStagingVacancy,
    /// The raw immutable `RelayedAdapterConfigV1` record.
    AdapterConfig,
    /// The finalized staging vacancy proving the adapter config is immutable.
    AdapterConfigStagingVacancy,
    /// The pre-existing RentCredit beneficiary.
    RentCredit,
    /// The Rent sysvar.
    RentSysvar,
    /// The Clock sysvar.
    ClockSysvar,
    /// The Instructions sysvar, used only to select the preceding precompile.
    InstructionsSysvar,
    /// The System Program.
    SystemProgram,
}

/// One ordered SDK-free account-role requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayAccountRoleV1 {
    name: RelayAccountNameV1,
    signer: bool,
    writable: bool,
}

impl RelayAccountRoleV1 {
    /// The semantic role name.
    pub const fn name(self) -> RelayAccountNameV1 {
        self.name
    }
    /// Whether the position must be a transaction signer.
    pub const fn is_signer(self) -> bool {
        self.signer
    }
    /// Whether the position must be writable.
    pub const fn is_writable(self) -> bool {
        self.writable
    }
}

const fn role(name: RelayAccountNameV1, signer: bool, writable: bool) -> RelayAccountRoleV1 {
    RelayAccountRoleV1 {
        name,
        signer,
        writable,
    }
}

const WORKER: RelayAccountRoleV1 = role(RelayAccountNameV1::Worker, true, true);
const MARKET_READ: RelayAccountRoleV1 = role(RelayAccountNameV1::Market, false, false);
const MARKET_WRITE: RelayAccountRoleV1 = role(RelayAccountNameV1::Market, false, true);
const RECORD: RelayAccountRoleV1 = role(RelayAccountNameV1::Record, false, true);
const MATERIAL: RelayAccountRoleV1 = role(RelayAccountNameV1::SourceMaterial, false, false);
const MATERIAL_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::SourceMaterialStagingVacancy,
    false,
    false,
);
const KEY_SET: RelayAccountRoleV1 = role(RelayAccountNameV1::RelayerKeySet, false, false);
const KEY_SET_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::RelayerKeySetStagingVacancy,
    false,
    false,
);
const CONFIG: RelayAccountRoleV1 = role(RelayAccountNameV1::AdapterConfig, false, false);
const CONFIG_STAGE: RelayAccountRoleV1 = role(
    RelayAccountNameV1::AdapterConfigStagingVacancy,
    false,
    false,
);
const CREDIT_READ: RelayAccountRoleV1 = role(RelayAccountNameV1::RentCredit, false, false);
const CREDIT_WRITE: RelayAccountRoleV1 = role(RelayAccountNameV1::RentCredit, false, true);
const RENT: RelayAccountRoleV1 = role(RelayAccountNameV1::RentSysvar, false, false);
const CLOCK: RelayAccountRoleV1 = role(RelayAccountNameV1::ClockSysvar, false, false);
const INSTRUCTIONS: RelayAccountRoleV1 = role(RelayAccountNameV1::InstructionsSysvar, false, false);
const SYSTEM: RelayAccountRoleV1 = role(RelayAccountNameV1::SystemProgram, false, false);

/// Exact record-creation and Market-registration frame.
///
/// Every raw record rides with the finalized staging vacancy that proves it is
/// immutable, which is the discipline `dclutch-sbf` already enforces for the
/// Source material.  `ProviderReleaseV1` needs no slot: it is carried inline by
/// the authenticated `SourceMaterialViewV1`.
pub const CREATE_RECORD_FRAME_V1: [RelayAccountRoleV1; 13] = [
    WORKER,
    MARKET_WRITE,
    RECORD,
    MATERIAL,
    MATERIAL_STAGE,
    KEY_SET,
    KEY_SET_STAGE,
    CONFIG,
    CONFIG_STAGE,
    CREDIT_READ,
    RENT,
    CLOCK,
    SYSTEM,
];

/// Exact append frame; the Ed25519 precompile rides immediately before it.
///
/// The adapter configuration is deliberately absent.  An append needs the
/// account set (which the record already persists) and the key set (to place
/// the signer); the staleness join needs the *attested mainnet* clock, which is
/// a decoded field of an attested account and is therefore a resolution-time
/// question, not a fill-time one.
pub const APPEND_OBSERVATION_FRAME_V1: [RelayAccountRoleV1; 8] = [
    WORKER,
    MARKET_READ,
    RECORD,
    KEY_SET,
    KEY_SET_STAGE,
    RENT,
    INSTRUCTIONS,
    CLOCK,
];

/// Exact seal frame; one signer per transaction.
pub const SEAL_RECORD_FRAME_V1: [RelayAccountRoleV1; 8] = [
    WORKER,
    MARKET_READ,
    RECORD,
    KEY_SET,
    KEY_SET_STAGE,
    RENT,
    INSTRUCTIONS,
    CLOCK,
];

/// Exact retirement, Market decrement and RentCredit closure frame.
pub const RETIRE_RECORD_FRAME_V1: [RelayAccountRoleV1; 4] =
    [WORKER, MARKET_WRITE, RECORD, CREDIT_WRITE];

/// Closed exact account-frame selector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RelayFrameKindV1 {
    /// [`CREATE_RECORD_FRAME_V1`].
    CreateRecord,
    /// [`APPEND_OBSERVATION_FRAME_V1`].
    AppendObservation,
    /// [`SEAL_RECORD_FRAME_V1`].
    SealRecord,
    /// [`RETIRE_RECORD_FRAME_V1`].
    RetireRecord,
}

/// Return the exact ordered roles for one relay operation.
pub const fn relay_frame_roles_v1(kind: RelayFrameKindV1) -> &'static [RelayAccountRoleV1] {
    match kind {
        RelayFrameKindV1::CreateRecord => &CREATE_RECORD_FRAME_V1,
        RelayFrameKindV1::AppendObservation => &APPEND_OBSERVATION_FRAME_V1,
        RelayFrameKindV1::SealRecord => &SEAL_RECORD_FRAME_V1,
        RelayFrameKindV1::RetireRecord => &RETIRE_RECORD_FRAME_V1,
    }
}

/// SDK-free observed account key and privileges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayAccountPrivilegeV1 {
    /// The observed account address.
    pub key: [u8; ADDRESS_BYTES],
    /// Whether the runtime reports it as a signer.
    pub is_signer: bool,
    /// Whether the runtime reports it as writable.
    pub is_writable: bool,
}

/// Validate exact count, privileges and complete no-alias policy for one frame.
///
/// The no-alias rule is not decoration: without it a caller could pass the
/// record account in the RentCredit position and close a live record's lamports
/// into itself, or pass the key-set record where the adapter config is expected
/// and have both content-ID checks read the same bytes.
pub fn validate_relay_frame_v1(
    kind: RelayFrameKindV1,
    accounts: &[RelayAccountPrivilegeV1],
) -> Result<()> {
    let roles = relay_frame_roles_v1(kind);
    if accounts.len() != roles.len() {
        return Err(Error::InvalidAccountFrame);
    }
    for (account, expected) in accounts.iter().zip(roles.iter()) {
        if account.is_signer != expected.is_signer()
            || account.is_writable != expected.is_writable()
        {
            return Err(Error::InvalidAccountFrame);
        }
    }
    for (index, account) in accounts.iter().enumerate() {
        for other in accounts.iter().skip(index.saturating_add(1)) {
            if account.key == other.key {
                return Err(Error::InvalidAccountFrame);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(kind: RelayFrameKindV1) -> [RelayAccountPrivilegeV1; 13] {
        let roles = relay_frame_roles_v1(kind);
        let mut built = [RelayAccountPrivilegeV1 {
            key: [0; 32],
            is_signer: false,
            is_writable: false,
        }; 13];
        for (index, expected) in roles.iter().enumerate() {
            let slot = built.get_mut(index).expect("within thirteen");
            let mut key = [0u8; 32];
            let first = key.get_mut(0).expect("first byte");
            *first = u8::try_from(index).expect("small") + 1;
            slot.key = key;
            slot.is_signer = expected.is_signer();
            slot.is_writable = expected.is_writable();
        }
        built
    }

    #[test]
    fn each_frame_accepts_exactly_its_own_shape() {
        for kind in [
            RelayFrameKindV1::CreateRecord,
            RelayFrameKindV1::AppendObservation,
            RelayFrameKindV1::SealRecord,
            RelayFrameKindV1::RetireRecord,
        ] {
            let built = frame(kind);
            let width = relay_frame_roles_v1(kind).len();
            let exact = built.get(..width).expect("prefix");
            assert_eq!(validate_relay_frame_v1(kind, exact), Ok(()));
            if width > 0 {
                let short = built.get(..width - 1).expect("short");
                assert_eq!(
                    validate_relay_frame_v1(kind, short),
                    Err(Error::InvalidAccountFrame)
                );
            }
        }
    }

    #[test]
    fn a_missing_worker_signature_refuses() {
        let mut built = frame(RelayFrameKindV1::SealRecord);
        built.get_mut(0).expect("worker").is_signer = false;
        let exact = built
            .get(..relay_frame_roles_v1(RelayFrameKindV1::SealRecord).len())
            .expect("prefix");
        assert_eq!(
            validate_relay_frame_v1(RelayFrameKindV1::SealRecord, exact),
            Err(Error::InvalidAccountFrame)
        );
    }

    #[test]
    fn a_writable_market_where_a_readonly_one_belongs_refuses() {
        let mut built = frame(RelayFrameKindV1::AppendObservation);
        built.get_mut(1).expect("market").is_writable = true;
        let exact = built
            .get(..relay_frame_roles_v1(RelayFrameKindV1::AppendObservation).len())
            .expect("prefix");
        assert_eq!(
            validate_relay_frame_v1(RelayFrameKindV1::AppendObservation, exact),
            Err(Error::InvalidAccountFrame)
        );
    }

    #[test]
    fn an_aliased_position_refuses_anywhere_in_the_frame() {
        let mut built = frame(RelayFrameKindV1::RetireRecord);
        let record_key = built.get(2).expect("record").key;
        built.get_mut(3).expect("credit").key = record_key;
        let exact = built.get(..4).expect("prefix");
        assert_eq!(
            validate_relay_frame_v1(RelayFrameKindV1::RetireRecord, exact),
            Err(Error::InvalidAccountFrame)
        );
    }
}
