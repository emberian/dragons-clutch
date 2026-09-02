//! Account construction and profile-driven physical packing.
//!
//! The campaign states each self-coordinate's *content* once, through a typed
//! constructor; the profile supplies everything topological: alias resolution,
//! physical order, writable/signer privileges, and the width every binding is
//! checked against. Funding is derived (rent minimum at the account's exact
//! width), never stated.

use dclutch_account_profile_contract::v2::{AccountProfileV2, PhysicalAccountDataGeometryV2};

use crate::profile_ops;
use solana_account::Account;
use solana_program::{instruction::AccountMeta, pubkey::Pubkey, rent::Rent};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program};

use crate::BuilderError;

/// One constructed chain account.
///
/// `account` is what the fixture *installs* (for an externally installed key,
/// an inert placeholder the enclosing ProgramTest never reads); `observed`,
/// when present, is what the chain will actually hold at execution — the view
/// the host projection engine must consume. Absent, the installed state is
/// the chain state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltAccountV1 {
    /// Exact account identity.
    pub key: Pubkey,
    /// Exact initial account state as installed.
    pub account: Account,
    /// Chain-true view for externally installed accounts.
    pub observed: Option<Account>,
}

impl BuiltAccountV1 {
    /// The account state the chain will hold at execution.
    #[must_use]
    pub fn chain_view(&self) -> &Account {
        self.observed.as_ref().unwrap_or(&self.account)
    }

    /// The same binding with an explicit chain-true view.
    #[must_use]
    pub fn with_observed(mut self, observed: Account) -> Self {
        self.observed = Some(observed);
        self
    }
}

/// Rent-funded data account owned by `owner`.
pub fn data_account(rent: &Rent, key: Pubkey, owner: Pubkey, data: Vec<u8>) -> BuiltAccountV1 {
    let lamports = if data.is_empty() {
        0
    } else {
        rent.minimum_balance(data.len())
    };
    BuiltAccountV1 {
        key,
        account: Account {
            lamports,
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        },
        observed: None,
    }
}

/// Vacant System-owned account (a staging cursor, an authority, a to-be-created
/// state).
pub fn vacant(key: Pubkey) -> BuiltAccountV1 {
    BuiltAccountV1 {
        key,
        account: Account {
            lamports: 0,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
        observed: None,
    }
}

/// Empty account with an explicit owner (a sysvar slot, external ProgramData).
pub fn external(key: Pubkey, owner: Pubkey) -> BuiltAccountV1 {
    BuiltAccountV1 {
        key,
        account: Account {
            lamports: 0,
            data: Vec::new(),
            owner,
            executable: false,
            rent_epoch: 0,
        },
        observed: None,
    }
}

/// Executable upgradeable-loader program account placeholder.
pub fn program(key: Pubkey) -> BuiltAccountV1 {
    BuiltAccountV1 {
        key,
        account: Account {
            lamports: 1,
            data: Vec::new(),
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
        },
        observed: None,
    }
}

/// The logical runtime frame under construction: one optional binding per
/// logical coordinate. Alias coordinates stay unbound; the packer resolves
/// them through their profile representative.
#[derive(Clone, Debug)]
pub struct LogicalFrameV1 {
    slots: Vec<Option<BuiltAccountV1>>,
}

impl LogicalFrameV1 {
    /// A frame of `count` unbound coordinates.
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            slots: vec![None; count],
        }
    }

    /// Number of logical coordinates.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the frame has no coordinates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Bind one self coordinate. Rebinding with different content refuses.
    pub fn bind(&mut self, coordinate: usize, value: BuiltAccountV1) -> Result<(), BuilderError> {
        let slot = self
            .slots
            .get_mut(coordinate)
            .ok_or(BuilderError::Binding(line!()))?;
        match slot {
            Some(existing) if *existing != value => Err(BuilderError::Binding(line!())),
            _ => {
                *slot = Some(value);
                Ok(())
            }
        }
    }

    /// Replace one coordinate's binding (the adoption path).
    pub fn adopt(&mut self, coordinate: usize, value: BuiltAccountV1) -> Result<(), BuilderError> {
        *self
            .slots
            .get_mut(coordinate)
            .ok_or(BuilderError::Binding(line!()))? = Some(value);
        Ok(())
    }

    /// The binding at one coordinate, if bound.
    #[must_use]
    pub fn get(&self, coordinate: usize) -> Option<&BuiltAccountV1> {
        self.slots.get(coordinate).and_then(Option::as_ref)
    }

    /// Resolve one coordinate through its profile representative.
    pub fn resolve(
        &self,
        profile: AccountProfileV2<'_>,
        tail_count: u32,
        spans: &[u32],
        coordinate: usize,
    ) -> Result<&BuiltAccountV1, BuilderError> {
        let representative = profile_ops::representative(profile, tail_count, spans, coordinate)?;
        self.get(representative)
            .ok_or(BuilderError::Binding(line!()))
    }
}

/// One packed physical account with its derived privileges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedAccountV1 {
    /// The account.
    pub built: BuiltAccountV1,
    /// Derived instruction meta (order, writable, signer).
    pub meta: AccountMeta,
    /// Whether late-child refusal must preserve this account byte-for-byte:
    /// derived as writable-and-not-signer.
    pub snapshot: bool,
}

/// Pack the logical frame into profile order with derived privileges.
///
/// Every physical representative must be bound; widths are validated against
/// the profile's declared data geometry; privileges come from the profile and
/// are never stated by the campaign.
pub fn pack_frame(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    spans: &[u32],
    frame: &LogicalFrameV1,
) -> Result<Vec<PackedAccountV1>, BuilderError> {
    let logical_count = profile_ops::logical_count(profile, tail_count, spans)?;
    if frame.len() != logical_count {
        return Err(BuilderError::Profile(line!()));
    }
    let physical_count = profile_ops::physical_count(profile, tail_count, spans)?;
    let mut packed: Vec<Option<BuiltAccountV1>> = vec![None; physical_count];
    for coordinate in 0..logical_count {
        let value = frame.resolve(profile, tail_count, spans, coordinate)?;
        let ordinal = profile_ops::ordinal(profile, tail_count, spans, coordinate)?;
        match packed
            .get_mut(ordinal)
            .ok_or(BuilderError::Profile(line!()))?
        {
            Some(existing) if existing != value => return Err(BuilderError::Binding(line!())),
            Some(_) => {}
            slot @ None => *slot = Some(value.clone()),
        }
    }
    packed
        .into_iter()
        .enumerate()
        .map(|(ordinal, value)| {
            let built = value.ok_or(BuilderError::Binding(line!()))?;
            let geometry = profile_ops::geometry(profile, tail_count, spans, ordinal)?;
            // Width is a fact about what the chain holds at execution, so an
            // externally installed account validates its chain view, not the
            // inert install placeholder.
            validate_width(geometry.data(), built.chain_view().data.len()).inspect_err(
                |_| {
                    std::eprintln!(
                        "pack width refused at ordinal {ordinal} (representative {}): observed {} declared {:?}",
                        geometry.logical_representative(),
                        built.chain_view().data.len(),
                        geometry.data(),
                    );
                },
            )?;
            let privileges = geometry.privileges();
            let meta = AccountMeta {
                pubkey: built.key,
                is_signer: privileges.signer(),
                is_writable: privileges.writable(),
            };
            let snapshot = privileges.writable() && !privileges.signer();
            Ok(PackedAccountV1 {
                built,
                meta,
                snapshot,
            })
        })
        .collect()
}

fn validate_width(
    geometry: PhysicalAccountDataGeometryV2,
    observed: usize,
) -> Result<(), BuilderError> {
    match geometry {
        PhysicalAccountDataGeometryV2::Exact { bytes } => {
            if observed == bytes {
                Ok(())
            } else {
                Err(BuilderError::Binding(line!()))
            }
        }
        PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => {
            if observed == 0 || observed == live_bytes {
                Ok(())
            } else {
                Err(BuilderError::Binding(line!()))
            }
        }
        PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
            if observed >= minimum_bytes {
                Ok(())
            } else {
                Err(BuilderError::Binding(line!()))
            }
        }
        PhysicalAccountDataGeometryV2::Opaque => Ok(()),
    }
}

/// Upgradeable-loader program binding whose chain view is the real 36-byte
/// Program record (`tag 2` plus the given ProgramData address).
pub fn program_with_view(key: Pubkey, programdata: Pubkey) -> BuiltAccountV1 {
    let mut data = Vec::with_capacity(36);
    data.extend_from_slice(&2_u32.to_le_bytes());
    data.extend_from_slice(programdata.as_ref());
    program(key).with_observed(Account {
        lamports: 1,
        data,
        owner: bpf_loader_upgradeable::ID,
        executable: true,
        rent_epoch: 0,
    })
}

/// [`program_with_view`] at the canonical Loader-derived ProgramData address.
pub fn program_with_deployed_view(key: Pubkey) -> BuiltAccountV1 {
    let programdata = Pubkey::find_program_address(&[key.as_ref()], &bpf_loader_upgradeable::ID).0;
    program_with_view(key, programdata)
}

/// The bank's registered name for the System program builtin.
///
/// One author for a string the bank owns and the campaign cannot install. It
/// is asserted against a live bank by the frame control every admitted
/// campaign runs, so a runtime that renames its builtin goes red naming this
/// constant rather than at the far end of a route.
pub const SYSTEM_PROGRAM_BUILTIN_NAME_V1: &str = "solana_system_program";

/// Builtin program binding whose chain view is the account genesis writes.
///
/// A builtin is not a deployed program: the bank owns it through the NATIVE
/// loader and its data is the registered name, not a 36-byte upgradeable
/// Loader `Program` record. It is always externally installed -- `bundle.rs`
/// lists `system_program::ID` among the external candidates -- so the campaign
/// states only the observation, and stating it wrong is invisible everywhere
/// except one place: `runtime_observations_digest` is a field of
/// `AdmittedInvocationContextV3`, so a mismodelled builtin makes the host's
/// scratch pages and the chain's request carry different context digests and
/// the admitted route refuses `0x4018 AdmittedTransport` with nothing naming a
/// coordinate.
///
/// Measured 2026-09-02 on General `OpenBatch` at N=2, which reached exactly
/// that: the campaign bound `program_with_deployed_view(system_program::ID)`,
/// claiming 36 bytes owned by `BPFLoaderUpgradeab1e...`, where the bank holds
/// 21 bytes of `solana_system_program` owned by `NativeLoader111...`.
#[must_use]
pub fn builtin_program(key: Pubkey, name: &str) -> BuiltAccountV1 {
    BuiltAccountV1 {
        key,
        account: Account {
            lamports: 1,
            data: name.as_bytes().to_vec(),
            owner: native_loader::ID,
            executable: true,
            rent_epoch: 0,
        },
        observed: None,
    }
}

/// The System program exactly as the bank holds it.
#[must_use]
pub fn system_program_builtin() -> BuiltAccountV1 {
    builtin_program(system_program::ID, SYSTEM_PROGRAM_BUILTIN_NAME_V1)
}

/// External data account whose chain view has the given exact content.
pub fn external_with_view(key: Pubkey, owner: Pubkey, observed_data: Vec<u8>) -> BuiltAccountV1 {
    external(key, owner).with_observed(Account {
        lamports: 1,
        data: observed_data,
        owner,
        executable: false,
        rent_epoch: 0,
    })
}

/// The Rent sysvar's canonical 17 serialized bytes for one rent schedule.
#[must_use]
#[allow(deprecated)]
pub fn rent_sysvar_bytes(rent: &Rent) -> Vec<u8> {
    let mut data = Vec::with_capacity(17);
    data.extend_from_slice(&rent.lamports_per_byte_year.to_le_bytes());
    data.extend_from_slice(&rent.exemption_threshold.to_le_bytes());
    data.push(rent.burn_percent);
    data
}
