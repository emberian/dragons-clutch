//! Finalized-chain unsigned material for current General action 47.
//!
//! This constructor owns the exact 81/82-role atomic General/Product/Source
//! retirement ABI. It accepts named, fixed-width observations rather than a
//! caller-shaped account vector, derives the sole 64-byte selector from the
//! observed terminal Epoch and indexed root, and requires one finalized ALT
//! containing every nonsigner role before producing an unsigned v0 message.

use crate::action_material::StructuredAddressLookupTableV1;
use crate::rpc_index::{
    CanonicalFamily, CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount,
    RpcCommitment,
};
use crate::transaction_builder::{
    general_action47_role_writable_v1, ConstructionError, ExactEquation, IntegerUnit,
    OwnedInstructionDraft, ProtocolTransactionBuilder, SemanticOwner, TransactionTransport,
    UnsignedProtocolTransaction, GENERAL_ACTION47_FAILED_ACCOUNT_COUNT_V1,
    GENERAL_ACTION47_SUCCESSFUL_ACCOUNT_COUNT_V1,
};
use clutch_general_v2_contract::{
    CountedSettlementRootSelectorV1, GeneralEpochPhaseV1, GeneralEpochV6AccountV1,
    Id32, MarketBindingV5, MarketRuntimeV3AccountV1,
    IndexedSettlementRootV1AccountV1, COUNTED_SETTLEMENT_ROOT_SELECTOR_BYTES,
};
use clutch_product_series::{
    MarketLifecyclePhaseV3, SeriesFundingPhaseV5, SeriesMarketLinkPhaseV3,
};
use clutch_retirement::{PositionAccountV3, PositionLifecycleV3, PositionPurposeV3};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesFundingAccountV5, SeriesMarketLinkAccountV3,
    SeriesRegistryAccountV4,
};
use clutch_solana_layout::registry::{ExtensionFamily, GeneralV2Action};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;

pub const GENERAL_ACTION47_VALIDITY_SLOTS_V1: u64 = 32;
pub const GENERAL_ACTION47_SUCCESSFUL_ROLES_V1: usize =
    GENERAL_ACTION47_SUCCESSFUL_ACCOUNT_COUNT_V1;
pub const GENERAL_ACTION47_FAILED_ROLES_V1: usize = GENERAL_ACTION47_FAILED_ACCOUNT_COUNT_V1;

/// Stable labels for the complete failed-source ABI. The successful ABI is
/// the exact 81-role prefix and has no trailing Source terminal account.
pub const GENERAL_ACTION47_ROLE_LABELS_V1: [&str; GENERAL_ACTION47_FAILED_ROLES_V1] = [
    "market-binding-v5",
    "market-runtime-v3",
    "product-root-v3",
    "series-link-v3",
    "series-funding-v5",
    "series-registry-v4",
    "registry-program",
    "registry-programdata",
    "registry-release",
    "capability-profile",
    "source-release",
    "compiler-bundle-v7",
    "market-instance-v2",
    "revenue-policy-record-v2",
    "revenue-policy-preimage-v2",
    "series-plan-v5",
    "series-funding-terms-v2",
    "product-template-v4",
    "native-claim-basis-v1",
    "recovery-policy-v1",
    "price-measure-policy-v1",
    "market-genesis-v2",
    "funding-quote-v6",
    "attachment-plan-v6",
    "collateral-refund",
    "neutral-collateral",
    "lamport-refund",
    "neutral-lamport",
    "collateral-authority",
    "realm",
    "collateral-profile",
    "collateral-policy",
    "collateral-mint",
    "token-program",
    "token-programdata",
    "system-program",
    "rent-sysvar",
    "registry-release-lamport-vault",
    "capability-profile-lamport-vault",
    "compiler-bundle-lamport-vault",
    "funding-quote-lamport-vault",
    "attachment-plan-lamport-vault",
    "series-plan-lamport-vault",
    "registry-release-collateral-vault",
    "capability-profile-collateral-vault",
    "compiler-bundle-collateral-vault",
    "funding-quote-collateral-vault",
    "attachment-plan-collateral-vault",
    "claim-ledger",
    "hoard",
    "hoard-token",
    "hoard-authority",
    "foundation-vault",
    "product-lifecycle-replay-v3",
    "source-adapter-program",
    "source-adapter-programdata",
    "source-parser-program",
    "source-parser-programdata",
    "source-parser-config",
    "source-spec",
    "source-work-schedule",
    "source-custody",
    "failure-admission",
    "failure-runtime",
    "failure-interval-cell",
    "failure-interval-history",
    "failure-permanent-replay",
    "failure-liveness-policy",
    "indexed-settlement-root",
    "general-epoch-v6",
    "candidate-window-v5",
    "fee-closure-manifest",
    "fee-terminal-receipt",
    "indexed-root-rent-payer",
    "fee-manifest-rent-payer",
    "fee-terminal-rent-payer",
    "treasury-service-ledger",
    "treasury-position-v3",
    "treasury-position-replay",
    "treasury-position-refund",
    "treasury-replay-refund",
    "failed-source-terminal-v3",
];

const OWNER_PACKAGE: &str =
    "clutch-general-v2-contract+clutch-product-series+clutch-source-plane-v3-runtime";
const OWNER_SCHEMA: &str = "dragons-clutch/operator/general-action47-material/v1";

pub type GeneralAction47MaterialResult<T> =
    core::result::Result<T, GeneralAction47MaterialError>;
type Result<T> = GeneralAction47MaterialResult<T>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralAction47MaterialError {
    CheckedRelease,
    ChainSnapshot,
    ChainAuthority,
    Arithmetic,
    Construction,
}

impl core::fmt::Display for GeneralAction47MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not admit current General action 47",
            Self::ChainSnapshot => "action-47 accounts are not one finalized snapshot",
            Self::ChainAuthority => "current General/Product/Source retirement authority refused",
            Self::Arithmetic => "action-47 finalized prestate arithmetic overflowed",
            Self::Construction => "release-bound General action-47 construction refused",
        })
    }
}

impl std::error::Error for GeneralAction47MaterialError {}

/// Compact current-General frame. Realm is intentionally absent: action 47
/// borrows the one Realm account from [`GeneralAction47PhysicalSnapshotV1`].
#[derive(Clone, Copy, Debug)]
pub struct GeneralAction47CurrentSnapshotV1<'a> {
    pub market_binding: &'a ObservedRpcAccount,
    pub market_runtime: &'a ObservedRpcAccount,
    pub product_root: &'a ObservedRpcAccount,
    pub series_link: &'a ObservedRpcAccount,
    pub series_funding: &'a ObservedRpcAccount,
    pub series_registry: &'a ObservedRpcAccount,
    pub registry_program: &'a ObservedRpcAccount,
    pub registry_programdata: &'a ObservedRpcAccount,
    pub registry_release: &'a ObservedRpcAccount,
    pub capability_profile: &'a ObservedRpcAccount,
    pub source_release: &'a ObservedRpcAccount,
    pub compiler_bundle: &'a ObservedRpcAccount,
    pub market_instance: &'a ObservedRpcAccount,
    pub revenue_record: &'a ObservedRpcAccount,
    pub revenue_preimage: &'a ObservedRpcAccount,
    /// SeriesPlan, FundingTerms, Template, NativeBasis, RecoveryPolicy,
    /// PricePolicy, Genesis, QuoteV6, AttachmentV6.
    pub artifacts: [&'a ObservedRpcAccount; 9],
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction47PhysicalSnapshotV1<'a> {
    pub collateral_refund: &'a ObservedRpcAccount,
    pub neutral_collateral: &'a ObservedRpcAccount,
    pub lamport_refund: &'a ObservedRpcAccount,
    pub neutral_lamport: &'a ObservedRpcAccount,
    pub collateral_authority: &'a ObservedRpcAccount,
    pub realm: &'a ObservedRpcAccount,
    pub collateral_profile: &'a ObservedRpcAccount,
    pub collateral_policy: &'a ObservedRpcAccount,
    pub collateral_mint: &'a ObservedRpcAccount,
    pub token_program: &'a ObservedRpcAccount,
    pub token_programdata: &'a ObservedRpcAccount,
    pub system_program: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
    pub lamport_vaults: [&'a ObservedRpcAccount; 6],
    pub collateral_vaults: [&'a ObservedRpcAccount; 5],
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction47LiabilitySnapshotV1<'a> {
    pub claim_ledger: &'a ObservedRpcAccount,
    pub hoard: &'a ObservedRpcAccount,
    pub hoard_token: &'a ObservedRpcAccount,
    pub hoard_authority: &'a ObservedRpcAccount,
    pub foundation_vault: &'a ObservedRpcAccount,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction47SourceSnapshotV1<'a> {
    pub lifecycle_replay: &'a ObservedRpcAccount,
    pub adapter_program: &'a ObservedRpcAccount,
    pub adapter_programdata: &'a ObservedRpcAccount,
    pub parser_program: &'a ObservedRpcAccount,
    pub parser_programdata: &'a ObservedRpcAccount,
    pub parser_config: &'a ObservedRpcAccount,
    pub source_spec: &'a ObservedRpcAccount,
    pub work_schedule: &'a ObservedRpcAccount,
    pub custody: &'a ObservedRpcAccount,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction47FailureSnapshotV1<'a> {
    pub admission: &'a ObservedRpcAccount,
    pub runtime: &'a ObservedRpcAccount,
    pub interval_cell: &'a ObservedRpcAccount,
    pub interval_history: &'a ObservedRpcAccount,
    pub permanent_replay: &'a ObservedRpcAccount,
    pub liveness_policy: &'a ObservedRpcAccount,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction47IndexedSnapshotV1<'a> {
    pub root: &'a ObservedRpcAccount,
    pub epoch: &'a ObservedRpcAccount,
    pub window: &'a ObservedRpcAccount,
    pub fee_manifest: &'a ObservedRpcAccount,
    pub fee_terminal: &'a ObservedRpcAccount,
    pub root_payer: &'a ObservedRpcAccount,
    pub manifest_payer: &'a ObservedRpcAccount,
    pub terminal_payer: &'a ObservedRpcAccount,
}

#[derive(Clone, Copy, Debug)]
pub struct GeneralAction47PositionSnapshotV1<'a> {
    pub treasury_service_ledger: &'a ObservedRpcAccount,
    pub position: &'a ObservedRpcAccount,
    pub replay: &'a ObservedRpcAccount,
    pub position_refund: &'a ObservedRpcAccount,
    pub replay_refund: &'a ObservedRpcAccount,
}

/// Every action role plus the compression-only finalized lookup table.
#[derive(Clone, Copy, Debug)]
pub struct GeneralAction47ChainSnapshotV1<'a> {
    pub current: GeneralAction47CurrentSnapshotV1<'a>,
    pub physical: GeneralAction47PhysicalSnapshotV1<'a>,
    pub liability: GeneralAction47LiabilitySnapshotV1<'a>,
    pub source: GeneralAction47SourceSnapshotV1<'a>,
    pub failure: GeneralAction47FailureSnapshotV1<'a>,
    pub indexed: GeneralAction47IndexedSnapshotV1<'a>,
    pub position: GeneralAction47PositionSnapshotV1<'a>,
    /// Present only for the failed-Source terminal branch. Its presence, not a
    /// caller boolean, selects the exact 82-role ABI.
    pub failed_source_terminal: Option<&'a ObservedRpcAccount>,
    pub address_lookup_table: &'a ObservedRpcAccount,
}

impl<'a> GeneralAction47ChainSnapshotV1<'a> {
    fn ordered(self) -> Vec<&'a ObservedRpcAccount> {
        let mut accounts = Vec::with_capacity(if self.failed_source_terminal.is_some() {
            GENERAL_ACTION47_FAILED_ROLES_V1
        } else {
            GENERAL_ACTION47_SUCCESSFUL_ROLES_V1
        });
        accounts.extend([
            self.current.market_binding,
            self.current.market_runtime,
            self.current.product_root,
            self.current.series_link,
            self.current.series_funding,
            self.current.series_registry,
            self.current.registry_program,
            self.current.registry_programdata,
            self.current.registry_release,
            self.current.capability_profile,
            self.current.source_release,
            self.current.compiler_bundle,
            self.current.market_instance,
            self.current.revenue_record,
            self.current.revenue_preimage,
        ]);
        accounts.extend(self.current.artifacts);
        accounts.extend([
            self.physical.collateral_refund,
            self.physical.neutral_collateral,
            self.physical.lamport_refund,
            self.physical.neutral_lamport,
            self.physical.collateral_authority,
            self.physical.realm,
            self.physical.collateral_profile,
            self.physical.collateral_policy,
            self.physical.collateral_mint,
            self.physical.token_program,
            self.physical.token_programdata,
            self.physical.system_program,
            self.physical.rent_sysvar,
        ]);
        accounts.extend(self.physical.lamport_vaults);
        accounts.extend(self.physical.collateral_vaults);
        accounts.extend([
            self.liability.claim_ledger,
            self.liability.hoard,
            self.liability.hoard_token,
            self.liability.hoard_authority,
            self.liability.foundation_vault,
            self.source.lifecycle_replay,
            self.source.adapter_program,
            self.source.adapter_programdata,
            self.source.parser_program,
            self.source.parser_programdata,
            self.source.parser_config,
            self.source.source_spec,
            self.source.work_schedule,
            self.source.custody,
            self.failure.admission,
            self.failure.runtime,
            self.failure.interval_cell,
            self.failure.interval_history,
            self.failure.permanent_replay,
            self.failure.liveness_policy,
            self.indexed.root,
            self.indexed.epoch,
            self.indexed.window,
            self.indexed.fee_manifest,
            self.indexed.fee_terminal,
            self.indexed.root_payer,
            self.indexed.manifest_payer,
            self.indexed.terminal_payer,
            self.position.treasury_service_ledger,
            self.position.position,
            self.position.replay,
            self.position.position_refund,
            self.position.replay_refund,
        ]);
        if let Some(terminal) = self.failed_source_terminal {
            accounts.push(terminal);
        }
        accounts
    }
}

#[derive(Clone, Debug)]
pub struct ChainDerivedGeneralAction47MaterialV1 {
    checked_release_key: String,
    program_id: Address,
    program_data: Address,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    observed_slot: u64,
    valid_before_slot: u64,
    failed_source: bool,
    selector: [u8; COUNTED_SETTLEMENT_ROOT_SELECTOR_BYTES],
    state_sha256: [u8; 32],
    total_writable_prestate_lamports: u128,
    ordered_accounts: Vec<AccountMeta>,
    lookup_table: StructuredAddressLookupTableV1,
}

impl ChainDerivedGeneralAction47MaterialV1 {
    pub const fn observed_slot(&self) -> u64 { self.observed_slot }
    pub const fn valid_before_slot(&self) -> u64 { self.valid_before_slot }
    pub const fn failed_source(&self) -> bool { self.failed_source }
    pub const fn selector(&self) -> [u8; COUNTED_SETTLEMENT_ROOT_SELECTOR_BYTES] {
        self.selector
    }
    pub const fn state_sha256(&self) -> [u8; 32] { self.state_sha256 }

    pub fn unsigned_instruction(
        &self,
        release: &IndexedProgramRelease,
    ) -> Result<OwnedInstructionDraft> {
        authenticate_material_release(self, release)?;
        OwnedInstructionDraft::checked_release_general_action47_v1(
            release,
            SemanticOwner {
                package: OWNER_PACKAGE.into(),
                schema: OWNER_SCHEMA.into(),
                release_sha256: self.release_manifest_sha256,
            },
            self.ordered_accounts.clone(),
            vec![ExactEquation {
                name: "Finalized action-47 writable prestate lamports are fixed by snapshot"
                    .into(),
                unit: IntegerUnit::Lamports,
                left: self.total_writable_prestate_lamports,
                right: self.total_writable_prestate_lamports,
            }],
            self.failed_source,
            self.selector,
        )
        .map_err(map_construction)
    }

    /// Compile the exact one-instruction, blockhash-free v0 transaction. The
    /// payer must be disjoint from every instruction role because action 47
    /// has no signer account and privilege union would widen its frozen ABI.
    pub fn unsigned_transaction(
        &self,
        release: &IndexedProgramRelease,
        payer: Address,
        transport: TransactionTransport,
    ) -> Result<UnsignedProtocolTransaction> {
        let draft = self.unsigned_instruction(release)?;
        ProtocolTransactionBuilder::new(
            payer,
            release.program_id,
            release.release_manifest_sha256,
            transport,
        )
        .and_then(|builder| {
            builder.build_exact_v0(
                draft,
                self.lookup_table.table(),
                self.lookup_table.observed_slot(),
                self.lookup_table.state_sha256(),
            )
        })
        .map_err(map_construction)
    }
}

/// Derive the sole current General action-47 request from one finalized exact
/// account snapshot. Sequence, action, selector, branch, roles, privileges,
/// and payload bytes are never supplied independently by the caller.
pub fn derive_general_action47_material_v1(
    release: &IndexedProgramRelease,
    snapshot: GeneralAction47ChainSnapshotV1<'_>,
) -> Result<ChainDerivedGeneralAction47MaterialV1> {
    authenticate_release(release)?;
    let ordered = snapshot.ordered();
    let failed_source = snapshot.failed_source_terminal.is_some();
    authenticate_provenance(release, &ordered)?;
    authenticate_lookup_provenance(snapshot.address_lookup_table, ordered[0])?;
    authenticate_role_shapes(release, &ordered, failed_source)?;
    authenticate_current_terminal_authority(release, snapshot)?;
    let lookup_table = StructuredAddressLookupTableV1::authenticate(snapshot.address_lookup_table)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let selector = selector(snapshot.indexed.epoch.address, snapshot.indexed.root.address)?;
    let ordered_accounts = ordered_metas(&ordered, failed_source)?;
    let total_writable_prestate_lamports = ordered_accounts
        .iter()
        .zip(ordered.iter())
        .try_fold(0_u128, |total, (meta, account)| {
            if meta.is_writable {
                total.checked_add(u128::from(account.lamports))
            } else {
                Some(total)
            }
        })
        .ok_or(GeneralAction47MaterialError::Arithmetic)?;
    let valid_before_slot = ordered[0]
        .provenance
        .slot
        .checked_add(GENERAL_ACTION47_VALIDITY_SLOTS_V1)
        .ok_or(GeneralAction47MaterialError::Arithmetic)?;
    Ok(ChainDerivedGeneralAction47MaterialV1 {
        checked_release_key: release.key(),
        program_id: release.program_id,
        program_data: release.program_data,
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        observed_slot: ordered[0].provenance.slot,
        valid_before_slot,
        failed_source,
        selector,
        state_sha256: snapshot_digest(&ordered),
        total_writable_prestate_lamports,
        ordered_accounts,
        lookup_table,
    })
}

fn authenticate_release(release: &IndexedProgramRelease) -> Result<()> {
    release
        .validate()
        .map_err(|_| GeneralAction47MaterialError::CheckedRelease)?;
    let coordinate = CanonicalIntentCoordinate {
        family_tag: ExtensionFamily::GeneralV2.tag(),
        family_version: ExtensionFamily::GeneralV2.version(),
        local_action: GeneralV2Action::CloseIndexedSettlementRoot.tag(),
    };
    if !release.families.contains(&CanonicalFamily::General)
        || release.enabled_intents.binary_search(&coordinate).is_err()
    {
        return Err(GeneralAction47MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_material_release(
    material: &ChainDerivedGeneralAction47MaterialV1,
    release: &IndexedProgramRelease,
) -> Result<()> {
    authenticate_release(release)?;
    if release.key() != material.checked_release_key
        || release.program_id != material.program_id
        || release.program_data != material.program_data
        || release.release_manifest_sha256 != material.release_manifest_sha256
        || release.capability_profile_id != material.capability_profile_id
    {
        return Err(GeneralAction47MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_provenance(
    release: &IndexedProgramRelease,
    accounts: &[&ObservedRpcAccount],
) -> Result<()> {
    let first = accounts
        .first()
        .ok_or(GeneralAction47MaterialError::ChainSnapshot)?
        .provenance
        .clone();
    let release_key = release.key();
    if first.commitment != RpcCommitment::Finalized
        || first.slot == 0
        || first.cluster_key.trim().is_empty()
        || first.release_key != release_key
        || accounts.iter().any(|account| {
            account.provenance.commitment != RpcCommitment::Finalized
                || account.provenance.slot != first.slot
                || account.provenance.cluster_key != first.cluster_key
                || account.provenance.release_key != release_key
        })
    {
        return Err(GeneralAction47MaterialError::ChainSnapshot);
    }
    Ok(())
}

fn authenticate_lookup_provenance(
    lookup: &ObservedRpcAccount,
    first: &ObservedRpcAccount,
) -> Result<()> {
    if lookup.provenance.commitment != RpcCommitment::Finalized
        || lookup.provenance.slot != first.provenance.slot
        || lookup.provenance.cluster_key != first.provenance.cluster_key
        || lookup.provenance.release_key != first.provenance.release_key
    {
        return Err(GeneralAction47MaterialError::ChainSnapshot);
    }
    Ok(())
}

fn authenticate_role_shapes(
    release: &IndexedProgramRelease,
    accounts: &[&ObservedRpcAccount],
    failed_source: bool,
) -> Result<()> {
    let expected = if failed_source {
        GENERAL_ACTION47_FAILED_ROLES_V1
    } else {
        GENERAL_ACTION47_SUCCESSFUL_ROLES_V1
    };
    if accounts.len() != expected {
        return Err(GeneralAction47MaterialError::ChainAuthority);
    }
    const EXECUTABLE_ROLES: [usize; 5] = [6, 33, 35, 54, 56];
    for (index, account) in accounts.iter().enumerate() {
        if account.address == Address::default()
            || account.executable != EXECUTABLE_ROLES.contains(&index)
            || account.lamports == 0
        {
            return Err(GeneralAction47MaterialError::ChainAuthority);
        }
    }
    // Current semantic owners and every mutable retirement state must be the
    // checked Clutch release. Token accounts, programs, sysvars, authorities,
    // refund destinations, and Realm-owned collateral facts remain external.
    const PROGRAM_OWNED: [core::ops::RangeInclusive<usize>; 7] = [
        0..=5,
        8..=13,
        15..=23,
        48..=49,
        52..=53,
        60..=72,
        76..=78,
    ];
    for (index, account) in accounts.iter().enumerate() {
        if PROGRAM_OWNED.iter().any(|range| range.contains(&index))
            && account.owner != release.program_id
        {
            return Err(GeneralAction47MaterialError::ChainAuthority);
        }
    }
    if let Some(terminal) = accounts.get(81) {
        if terminal.owner != release.program_id
            || terminal.data.len() != 1_248
            || terminal.data.get(..8) != Some(b"DCSPFB03".as_slice())
        {
            return Err(GeneralAction47MaterialError::ChainAuthority);
        }
    }
    Ok(())
}

fn authenticate_current_terminal_authority(
    release: &IndexedProgramRelease,
    snapshot: GeneralAction47ChainSnapshotV1<'_>,
) -> Result<()> {
    let binding = MarketBindingV5::decode(&snapshot.current.market_binding.data)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let runtime = MarketRuntimeV3AccountV1::decode(&snapshot.current.market_runtime.data)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let root = MarketLifecycleRootAccountV3::decode(&snapshot.current.product_root.data)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let link = SeriesMarketLinkAccountV3::decode(&snapshot.current.series_link.data)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let funding = SeriesFundingAccountV5::decode(&snapshot.current.series_funding.data)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let registry = SeriesRegistryAccountV4::decode(&snapshot.current.series_registry.data)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let indexed = IndexedSettlementRootV1AccountV1::decode(&snapshot.indexed.root.data)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let epoch = GeneralEpochV6AccountV1::decode(&snapshot.indexed.epoch.data)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let position = PositionAccountV3::decode(&snapshot.position.position.data)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    let relation = binding.base().base();
    let authority = binding.authority();
    let root_binding = root.state.binding_ref();
    let link_binding = link.state.binding_ref();
    if relation.market.bytes() != snapshot.current.market_runtime.address.to_bytes()
        || runtime.market_binding.bytes() != snapshot.current.market_binding.address.to_bytes()
        || runtime.market_instance_v2_id != relation.market_instance_v2_id
        || runtime.live_epoch_count().map_err(|_| GeneralAction47MaterialError::ChainAuthority)?
            != 1
        || authority.product_market_root_account().bytes()
            != snapshot.current.product_root.address.to_bytes()
        || authority.product_market_binding_v3_id().bytes()
            != root_binding.id().map_err(|_| GeneralAction47MaterialError::ChainAuthority)?.bytes()
        || authority.product_generation() != root_binding.generation
        || root_binding.market_instance_id.content_id().bytes()
            != relation.market_instance_v2_id.bytes()
        || root.state.phase() != MarketLifecyclePhaseV3::Active
        || root.state.live_series_links() != 1
        || authority.series_market_link_account().bytes()
            != snapshot.current.series_link.address.to_bytes()
        || authority.series_market_link_v3_id().bytes()
            != link_binding.id().map_err(|_| GeneralAction47MaterialError::ChainAuthority)?.bytes()
        || authority.series_ordinal() != link_binding.ordinal
        || link_binding.series_plan_id.content_id().bytes()
            != relation.series_plan_v5_id.bytes()
        || link_binding.funding_terms_id.content_id().bytes()
            != relation.series_funding_terms_v2_id.bytes()
        || link_binding.market_root_account_id.bytes()
            != snapshot.current.product_root.address.to_bytes()
        || link_binding.market_instance_id != root_binding.market_instance_id
        || link_binding.generation != root_binding.generation
        || link.state.phase() != SeriesMarketLinkPhaseV3::Active
        || authority.series_funding_v5_account().bytes()
            != snapshot.current.series_funding.address.to_bytes()
        || authority.compiler_bundle_v7_id().bytes()
            != link_binding.compiler_bundle_id.content_id().bytes()
        || authority.revenue_policy_record_account().bytes()
            != snapshot.current.revenue_record.address.to_bytes()
        || funding.state.phase != SeriesFundingPhaseV5::Closed
        || funding.state.series_plan_id != link_binding.series_plan_id
        || funding.state.funding_terms_id != link_binding.funding_terms_id
        || funding.state.compiler_bundle_id != link_binding.compiler_bundle_id
        || registry.series_plan_id != link_binding.series_plan_id
        || registry.funding_terms_id != link_binding.funding_terms_id
        || registry.compiler_bundle_id != link_binding.compiler_bundle_id
        || !registry.activation_consumed
        || !indexed.is_terminal()
        || indexed.base().epoch().bytes() != snapshot.indexed.epoch.address.to_bytes()
        || indexed.base().market().bytes() != snapshot.current.market_runtime.address.to_bytes()
        || indexed.base().market_binding().bytes()
            != snapshot.current.market_binding.address.to_bytes()
        || indexed.base().market_instance_v2_id() != relation.market_instance_v2_id
        || indexed.base().window().bytes() != snapshot.indexed.window.address.to_bytes()
        || indexed.base().epoch_generation() != epoch.generation
        || epoch.phase != GeneralEpochPhaseV1::Finalized
        || epoch.selected_candidate_count != 1
        || epoch.market_binding.bytes() != snapshot.current.market_binding.address.to_bytes()
        || epoch.market_runtime.bytes() != snapshot.current.market_runtime.address.to_bytes()
        || epoch.market_instance_v2_id != relation.market_instance_v2_id
        || epoch.window.bytes() != snapshot.indexed.window.address.to_bytes()
        || position.purpose() != PositionPurposeV3::General
        || position.lifecycle() != PositionLifecycleV3::CloseRequested
        || position.market_instance_id().bytes() != relation.market_instance_v2_id.bytes()
        || position.realm_id().bytes() != root_binding.realm_id.bytes()
        || position.collateral_policy_id().bytes() != root_binding.collateral_policy_id.bytes()
        || position.collateral_release_id().bytes() != root_binding.collateral_release_id.bytes()
        || position.owner().bytes() != authority.treasury_owner().bytes()
        || position.controller() != position.owner()
        || position.purpose_binding_id().bytes()
            != snapshot.current.market_runtime.address.to_bytes()
        || position.replay_account().bytes() != snapshot.position.replay.address.to_bytes()
        || position.cash_atoms() != 0
        || position.reserved_cash_atoms() != 0
        || position.native_eggs().iter().any(|atoms| *atoms != 0)
        || position.outstanding_reservations() != 0
        || position.rent().payer.bytes() != snapshot.position.position_refund.address.to_bytes()
        || relation.neutral_sink.bytes() != snapshot.physical.neutral_lamport.address.to_bytes()
        || authority.treasury_position_account().bytes()
            != snapshot.position.position.address.to_bytes()
        || authority.treasury_service_ledger_account().bytes()
            != snapshot.position.treasury_service_ledger.address.to_bytes()
        || snapshot.current.market_binding.owner != release.program_id
    {
        return Err(GeneralAction47MaterialError::ChainAuthority);
    }
    Ok(())
}

fn selector(epoch: Address, root: Address) -> Result<[u8; COUNTED_SETTLEMENT_ROOT_SELECTOR_BYTES]> {
    let mut bytes = [0_u8; COUNTED_SETTLEMENT_ROOT_SELECTOR_BYTES];
    bytes[..32].copy_from_slice(&epoch.to_bytes());
    bytes[32..].copy_from_slice(&root.to_bytes());
    CountedSettlementRootSelectorV1::decode(&bytes)
        .map_err(|_| GeneralAction47MaterialError::ChainAuthority)?;
    Ok(bytes)
}

fn ordered_metas(
    accounts: &[&ObservedRpcAccount],
    failed_source: bool,
) -> Result<Vec<AccountMeta>> {
    accounts
        .iter()
        .enumerate()
        .map(|(index, account)| {
            Ok(AccountMeta {
                pubkey: account.address,
                is_signer: false,
                is_writable: general_action47_role_writable_v1(index, failed_source)
                    .ok_or(GeneralAction47MaterialError::ChainAuthority)?,
            })
        })
        .collect()
}

fn snapshot_digest(accounts: &[&ObservedRpcAccount]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"dragons-clutch/operator/general-action47-finalized-snapshot/v1");
    hasher.update(accounts[0].provenance.slot.to_le_bytes());
    for account in accounts {
        hasher.update(account.address.to_bytes());
        hasher.update(account.owner.to_bytes());
        hasher.update(account.lamports.to_le_bytes());
        hasher.update([u8::from(account.executable)]);
        hasher.update((account.data.len() as u64).to_le_bytes());
        hasher.update(&account.data);
    }
    hasher.finalize().into()
}

fn map_construction(_error: ConstructionError) -> GeneralAction47MaterialError {
    GeneralAction47MaterialError::Construction
}
