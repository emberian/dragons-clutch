#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-ELF-only support for the joined recurring-Series lifecycle campaign.
//!
//! The support layer owns no protocol semantics and installs no native or mock
//! processor. It loads exact SBF ELFs, authenticates the compile-time selected
//! Series Shadow source manifest, fixes the five-route founding order, and
//! snapshots every account class that a deliberately late refusal must roll
//! back byte-for-byte.

use std::{env, fs, path::Path};

use dclutch_core_contract::ContentId;
use sha2::{Digest, Sha256};
use solana_account::Account;
use solana_program::{pubkey::Pubkey, rent::Rent};
use solana_program_test::{BanksClient, BanksClientError, ProgramTest};
use solana_sdk_ids::bpf_loader_upgradeable;

/// Exact SBF artifact filenames required by the joined Series campaign.
pub const SERIES_REAL_SBF_ARTIFACTS_V1: [&str; 7] = [
    "dclutch_registry_sbf.so",
    "dclutch_rent_sbf.so",
    "dclutch_trading_sbf.so",
    "dclutch_core_sbf.so",
    "dclutch_claims_sbf.so",
    "dclutch_custody_sbf.so",
    "dclutch_accelerator_sbf.so",
];

/// Exact global child-route order owned by the selected DCE5 Effect.
pub const SERIES_JOINED_FOUNDING_ROUTES_V1: [SeriesJoinedFoundingRouteV1; 5] = [
    SeriesJoinedFoundingRouteV1::LockHoardAndCloseSource,
    SeriesJoinedFoundingRouteV1::CoreFound,
    SeriesJoinedFoundingRouteV1::RealizeAndClose,
    SeriesJoinedFoundingRouteV1::ClaimsFoundingV5,
    SeriesJoinedFoundingRouteV1::CoreOpen,
];

const SERIES_SHADOW_ELF_ORDINAL: usize = 6;

/// Stable real-SBF harness refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesRealSbfHarnessErrorV1 {
    /// `SBF_OUT_DIR` or one required ELF was absent or unreadable.
    Elf,
    /// A selected source manifest or generated include was absent or malformed.
    Source,
    /// The host crate was not built with the deliberately selected include.
    NoSelectedRelease,
    /// Selected embedded bytes differed from the exact source manifest.
    ReleaseSubstitution,
    /// A rollback target was zero, duplicated, or omitted a required class.
    RollbackGeometry,
    /// One account differed after a transaction-level refusal.
    RollbackMismatch,
}

/// One global child route in the atomic founding transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesJoinedFoundingRouteV1 {
    /// Projected Custody moves all SeriesEscrow principal into future Hoard and
    /// closes the source vault and replay.
    LockHoardAndCloseSource,
    /// Core authenticates the lock receipt, creates the Market and permit, and
    /// emits the exact Found acknowledgement.
    CoreFound,
    /// Projected Custody realizes the Hoard against the now-live Market.
    RealizeAndClose,
    /// Claims FoundingV5 consumes both ordered Custody receipts.
    ClaimsFoundingV5,
    /// Core authenticates Claims and commits the final Open state.
    CoreOpen,
}

/// Program identities used when installing the seven exact real ELFs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRealSbfProgramIdsV1 {
    /// Current Registry program.
    pub registry: Pubkey,
    /// Current Rent program.
    pub rent: Pubkey,
    /// Current Trading program.
    pub trading: Pubkey,
    /// Current Core program.
    pub core: Pubkey,
    /// Current Claims program.
    pub claims: Pubkey,
    /// Current Custody program.
    pub custody: Pubkey,
    /// Deliberately selected stateless Series Shadow accelerator program.
    pub series_shadow: Pubkey,
}

impl SeriesRealSbfProgramIdsV1 {
    fn ordered(self) -> [Pubkey; SERIES_REAL_SBF_ARTIFACTS_V1.len()] {
        [
            self.registry,
            self.rent,
            self.trading,
            self.core,
            self.claims,
            self.custody,
            self.series_shadow,
        ]
    }
}

/// Exact bytes of all real executables required by one joined campaign.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesRealSbfElvesV1 {
    bytes: [Vec<u8>; SERIES_REAL_SBF_ARTIFACTS_V1.len()],
}

impl SeriesRealSbfElvesV1 {
    /// Load every real ELF from `SBF_OUT_DIR`; no processor fallback exists.
    pub fn load_from_environment() -> Result<Self, SeriesRealSbfHarnessErrorV1> {
        let directory = env::var_os("SBF_OUT_DIR").ok_or(SeriesRealSbfHarnessErrorV1::Elf)?;
        Self::load_from_directory(Path::new(&directory))
    }

    /// Load every exact real ELF from one build-only output directory.
    pub fn load_from_directory(directory: &Path) -> Result<Self, SeriesRealSbfHarnessErrorV1> {
        let mut bytes: [Vec<u8>; SERIES_REAL_SBF_ARTIFACTS_V1.len()] =
            core::array::from_fn(|_| Vec::new());
        for (ordinal, filename) in SERIES_REAL_SBF_ARTIFACTS_V1.iter().enumerate() {
            let elf =
                fs::read(directory.join(filename)).map_err(|_| SeriesRealSbfHarnessErrorV1::Elf)?;
            if elf.is_empty() {
                return Err(SeriesRealSbfHarnessErrorV1::Elf);
            }
            *bytes
                .get_mut(ordinal)
                .ok_or(SeriesRealSbfHarnessErrorV1::Elf)? = elf;
        }
        Ok(Self { bytes })
    }

    /// SHA-256 of the exact selected Series Shadow accelerator ELF.
    pub fn series_shadow_elf_digest(&self) -> [u8; 32] {
        digest(
            self.bytes
                .get(SERIES_SHADOW_ELF_ORDINAL)
                .map(Vec::as_slice)
                .unwrap_or_default(),
        )
    }

    /// Install every executable as one immutable upgradeable program.
    ///
    /// `ProgramTest` resolves each program by its exact artifact filename and
    /// `SBF_OUT_DIR`; `None` native processors are the only accepted form.
    pub fn install(
        &self,
        test: &mut ProgramTest,
        ids: SeriesRealSbfProgramIdsV1,
    ) -> Result<(), SeriesRealSbfHarnessErrorV1> {
        for ((filename, bytes), program) in SERIES_REAL_SBF_ARTIFACTS_V1
            .iter()
            .zip(self.bytes.iter())
            .zip(ids.ordered())
        {
            let name = filename
                .strip_suffix(".so")
                .ok_or(SeriesRealSbfHarnessErrorV1::Elf)?;
            test.add_upgradeable_program_to_genesis(name, &program);
            let programdata_bytes = immutable_programdata(bytes)?;
            test.add_account(
                programdata(program),
                Account {
                    lamports: Rent::default().minimum_balance(programdata_bytes.len()),
                    data: programdata_bytes,
                    owner: bpf_loader_upgradeable::ID,
                    executable: false,
                    rent_epoch: 0,
                },
            );
        }
        Ok(())
    }
}

/// Exact selected source/build evidence paired with the real Shadow ELF.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesSelectedShadowBuildEvidenceV1 {
    /// Digest of the hostile-decodable source manifest.
    pub source_manifest: ContentId,
    /// Digest of the generator-produced include payload.
    pub generated_include: ContentId,
    /// Domain-separated digest of all embedded artifact bytes.
    pub bundle: ContentId,
    /// SHA-256 of the exact real Shadow ELF.
    pub elf: [u8; 32],
}

/// Account class whose complete prestate must survive a late refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesRollbackRoleV1 {
    /// Trading-owned recurring-Series root.
    SeriesRoot,
    /// Trading-owned Ticket replay.
    Ticket,
    /// Core Market, including vacancy before Found.
    Market,
    /// One-shot Core founding permit.
    Permit,
    /// Claims aggregate, Position, admission, or linked evidence.
    Claims,
    /// Projected/normal Custody replay, vault, or authority-owned state.
    Custody,
    /// Ordered Trading FundingState.
    Funding,
    /// Market+generation LifecycleRentCreditV2 sink.
    LifecycleRentCredit,
}

/// One required rollback observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRollbackTargetV1 {
    /// Semantic account class.
    pub role: SeriesRollbackRoleV1,
    /// Exact physical account identity.
    pub key: Pubkey,
}

/// Canonical unique rollback target set for late Claims/Open refusal evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesRollbackSetV1 {
    targets: Vec<SeriesRollbackTargetV1>,
}

impl SeriesRollbackSetV1 {
    /// Validate an exact target set containing every state class.
    pub fn new(targets: Vec<SeriesRollbackTargetV1>) -> Result<Self, SeriesRealSbfHarnessErrorV1> {
        if targets.is_empty()
            || targets.iter().any(|target| target.key == Pubkey::default())
            || targets.iter().enumerate().any(|(ordinal, target)| {
                targets
                    .get(..ordinal)
                    .is_some_and(|prior| prior.iter().any(|other| other.key == target.key))
            })
            || !REQUIRED_ROLLBACK_ROLES
                .iter()
                .all(|role| targets.iter().any(|target| target.role == *role))
        {
            return Err(SeriesRealSbfHarnessErrorV1::RollbackGeometry);
        }
        Ok(Self { targets })
    }

    /// Ordered unique physical targets.
    pub fn targets(&self) -> &[SeriesRollbackTargetV1] {
        &self.targets
    }

    /// Snapshot complete account state, including absence, owner, lamports,
    /// executable bit, rent epoch, and every data byte.
    pub async fn snapshot(
        &self,
        banks: &mut BanksClient,
    ) -> Result<SeriesRollbackSnapshotV1, BanksClientError> {
        let mut accounts = Vec::with_capacity(self.targets.len());
        for target in &self.targets {
            accounts.push(SeriesRollbackAccountV1 {
                target: *target,
                account: banks.get_account(target.key).await?,
            });
        }
        Ok(SeriesRollbackSnapshotV1 { accounts })
    }
}

const REQUIRED_ROLLBACK_ROLES: [SeriesRollbackRoleV1; 8] = [
    SeriesRollbackRoleV1::SeriesRoot,
    SeriesRollbackRoleV1::Ticket,
    SeriesRollbackRoleV1::Market,
    SeriesRollbackRoleV1::Permit,
    SeriesRollbackRoleV1::Claims,
    SeriesRollbackRoleV1::Custody,
    SeriesRollbackRoleV1::Funding,
    SeriesRollbackRoleV1::LifecycleRentCredit,
];

/// One complete account observation in a rollback snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesRollbackAccountV1 {
    /// Semantic role and exact account identity.
    pub target: SeriesRollbackTargetV1,
    /// Complete account or exact absence.
    pub account: Option<Account>,
}

/// Ordered complete state used for byte-exact rollback comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesRollbackSnapshotV1 {
    accounts: Vec<SeriesRollbackAccountV1>,
}

impl SeriesRollbackSnapshotV1 {
    /// Exact ordered account observations.
    pub fn accounts(&self) -> &[SeriesRollbackAccountV1] {
        &self.accounts
    }

    /// Require complete equality after a deliberately late transaction refusal.
    pub fn require_byte_exact_rollback(
        &self,
        after: &Self,
    ) -> Result<(), SeriesRealSbfHarnessErrorV1> {
        if self == after {
            Ok(())
        } else {
            Err(SeriesRealSbfHarnessErrorV1::RollbackMismatch)
        }
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn immutable_programdata(elf: &[u8]) -> Result<Vec<u8>, SeriesRealSbfHarnessErrorV1> {
    let total = 45_usize
        .checked_add(elf.len())
        .ok_or(SeriesRealSbfHarnessErrorV1::Elf)?;
    let mut bytes = vec![0; total];
    bytes
        .get_mut(..4)
        .ok_or(SeriesRealSbfHarnessErrorV1::Elf)?
        .copy_from_slice(&3_u32.to_le_bytes());
    bytes
        .get_mut(4..12)
        .ok_or(SeriesRealSbfHarnessErrorV1::Elf)?
        .copy_from_slice(&0_u64.to_le_bytes());
    *bytes.get_mut(12).ok_or(SeriesRealSbfHarnessErrorV1::Elf)? = 0;
    bytes
        .get_mut(45..)
        .ok_or(SeriesRealSbfHarnessErrorV1::Elf)?
        .copy_from_slice(elf);
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use dclutch_accelerator_sbf::series::release::selected_series_shadow_release_v1;

    use super::*;

    fn target(role: SeriesRollbackRoleV1, byte: u8) -> SeriesRollbackTargetV1 {
        SeriesRollbackTargetV1 {
            role,
            key: Pubkey::new_from_array([byte; 32]),
        }
    }

    fn complete_targets() -> Vec<SeriesRollbackTargetV1> {
        REQUIRED_ROLLBACK_ROLES
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, role)| {
                target(
                    role,
                    u8::try_from(ordinal + 1).expect("bounded role ordinal"),
                )
            })
            .collect()
    }

    #[test]
    fn exact_route_order_cannot_be_relabelled() {
        assert_eq!(
            SERIES_JOINED_FOUNDING_ROUTES_V1,
            [
                SeriesJoinedFoundingRouteV1::LockHoardAndCloseSource,
                SeriesJoinedFoundingRouteV1::CoreFound,
                SeriesJoinedFoundingRouteV1::RealizeAndClose,
                SeriesJoinedFoundingRouteV1::ClaimsFoundingV5,
                SeriesJoinedFoundingRouteV1::CoreOpen,
            ]
        );
    }

    #[test]
    fn rollback_set_requires_every_class_and_unique_keys() {
        let complete = complete_targets();
        assert!(SeriesRollbackSetV1::new(complete.clone()).is_ok());

        let mut missing = complete.clone();
        missing.pop();
        assert_eq!(
            SeriesRollbackSetV1::new(missing),
            Err(SeriesRealSbfHarnessErrorV1::RollbackGeometry)
        );

        let mut duplicate = complete;
        let first = duplicate
            .first()
            .map(|target| target.key)
            .unwrap_or_default();
        if let Some(last) = duplicate.last_mut() {
            last.key = first;
        }
        assert_eq!(
            SeriesRollbackSetV1::new(duplicate),
            Err(SeriesRealSbfHarnessErrorV1::RollbackGeometry)
        );
    }

    #[test]
    fn unselected_host_build_cannot_claim_release_evidence() {
        assert_eq!(selected_series_shadow_release_v1(), Ok(None));
    }
}
