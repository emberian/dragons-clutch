//! Resumable orchestration for the transport-neutral RPC/index plane.
//!
//! The engine emits exact JSON-RPC requests and accepts bounded decoded
//! responses. It never opens a socket. Program-account notifications remain
//! buffered until the corresponding slot is both block-identified and frozen,
//! because a slot number alone is not a fork identity.

use crate::account_index::{
    AccountIndexError, CanonicalAccountIndex, CanonicalDecoderContext, IndexCapacity,
};
use crate::action_material::CanonicalActionMaterialErrorV1;
use crate::direct_action8_material::{
    enumerate_direct_action8_material_v2, join_direct_action8_finalized_snapshots_v2,
    plan_direct_action8_context_snapshot_v2, DirectAction8OperatorBatchV2,
};
use crate::collateral_release_catalog::CurrentCollateralReleaseCatalogV1;
use crate::collateral_release_catalog::AuthenticatedCurrentCollateralReleaseV1;
use crate::dealer_terminal_material::{
    enumerate_dealer_terminal_material_v1, join_dealer_terminal_snapshots_v1,
    plan_dealer_terminal_snapshot_v1, DealerTerminalOperatorBatchV1,
};
use crate::rpc_index::{
    decode_block_notification, decode_finalized_exact_account_snapshot_v1,
    decode_program_notification, decode_program_scan_snapshot_v1, decode_response_result,
    decode_root_notification, decode_slot_update_notification,
    decode_subscription_registration, notification_subscription_id, program_scan_context_slot,
    FinalizedAccountSnapshotV1, FinalizedExactAccountSnapshotRequestV1, ObservedRpcAccount,
    ObservedRpcProgramUpdate, ObservedSlotUpdateKind, PlannedRpcRequest,
    RpcAccountRemovalKind, RpcIndexError, RpcIndexPlan, RpcRequestPurpose,
};
use crate::transaction_builder::ProtocolTransactionBuilder;
use crate::workflow_graph::ExplicitOperatorReleaseManifest;
use serde_json::Value;
use solana_address::Address;
use std::collections::{BTreeMap, BTreeSet};

pub type Result<T> = core::result::Result<T, RpcIndexEngineError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpcIndexEngineError {
    Rpc(RpcIndexError),
    Account(AccountIndexError),
    DirectActionMaterial(CanonicalActionMaterialErrorV1),
    UnknownRequest,
    DuplicateResponse,
    UnknownSubscription,
    DuplicateSubscription,
    CapacityExceeded,
    SequenceExhausted,
}

impl core::fmt::Display for RpcIndexEngineError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Rpc(error) => write!(formatter, "RPC index refused input: {error}"),
            Self::Account(error) => write!(formatter, "account index refused input: {error}"),
            Self::DirectActionMaterial(error) => {
                write!(formatter, "Direct action material refused input: {error}")
            }
            Self::UnknownRequest => formatter.write_str("RPC response has no planned request"),
            Self::DuplicateResponse => {
                formatter.write_str("RPC request already has an admitted response")
            }
            Self::UnknownSubscription => {
                formatter.write_str("notification names an unknown subscription")
            }
            Self::DuplicateSubscription => {
                formatter.write_str("subscription coordinate is already bound")
            }
            Self::CapacityExceeded => {
                formatter.write_str("pending processed account capacity is exhausted")
            }
            Self::SequenceExhausted => formatter.write_str("receive sequence is exhausted"),
        }
    }
}

impl std::error::Error for RpcIndexEngineError {}

impl From<RpcIndexError> for RpcIndexEngineError {
    fn from(value: RpcIndexError) -> Self {
        Self::Rpc(value)
    }
}

impl From<AccountIndexError> for RpcIndexEngineError {
    fn from(value: AccountIndexError) -> Self {
        Self::Account(value)
    }
}

impl From<CanonicalActionMaterialErrorV1> for RpcIndexEngineError {
    fn from(value: CanonicalActionMaterialErrorV1) -> Self {
        Self::DirectActionMaterial(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RpcIndexEngineEvent {
    FinalizedScanAdmitted {
        release_key: String,
        account_count: usize,
    },
    FinalizedExactSnapshotAdmitted {
        request_id: u64,
        account_count: usize,
        slot: u64,
    },
    SubscriptionBound {
        request_id: u64,
        subscription_id: u64,
    },
    AccountIndexed {
        address: Address,
        slot: u64,
    },
    AccountBuffered {
        address: Address,
        slot: u64,
    },
    AccountRemovalBuffered {
        address: Address,
        slot: u64,
        kind: RpcAccountRemovalKind,
    },
    AccountRemoved {
        address: Address,
        observed_owner: Address,
        slot: u64,
        kind: RpcAccountRemovalKind,
        projection_withdrawn: bool,
    },
    BufferedAccountsDropped {
        slot: u64,
        account_count: usize,
    },
    IndexedAccountsRolledBack {
        slot: u64,
        account_count: usize,
    },
    SlotObserved {
        slot: u64,
    },
    SlotUpdated {
        slot: u64,
        kind: ObservedSlotUpdateKind,
    },
    RootFinalized {
        slot: u64,
    },
    RootDeferred {
        slot: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessedReconnectRollback {
    pub indexed_versions_removed: usize,
    pub buffered_accounts_removed: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RpcIndexEngineStatus {
    pub bootstrap_complete: bool,
    pub remaining_scans: usize,
    pub remaining_subscription_registrations: usize,
    pub active_subscriptions: usize,
    pub pending_accounts: usize,
    pub pending_account_bytes: usize,
    pub pending_root: Option<u64>,
    pub next_receive_sequence: u64,
}

/// Owned index plus all resumable transport bindings.
pub struct RpcIndexEngine {
    index: CanonicalAccountIndex,
    scan_requests: BTreeMap<u64, PlannedRpcRequest>,
    subscription_requests: BTreeMap<u64, PlannedRpcRequest>,
    completed_scans: BTreeSet<u64>,
    finalized_scan_snapshots: BTreeMap<String, FinalizedAccountSnapshotV1>,
    exact_snapshot_requests: BTreeMap<u64, FinalizedExactAccountSnapshotRequestV1>,
    exact_snapshots: BTreeMap<u64, FinalizedAccountSnapshotV1>,
    registered_requests: BTreeSet<u64>,
    active_subscriptions: BTreeMap<u64, PlannedRpcRequest>,
    pending_accounts: BTreeMap<u64, Vec<ObservedRpcProgramUpdate>>,
    pending_account_count: usize,
    pending_account_bytes: usize,
    pending_root: Option<u64>,
    next_receive_sequence: u64,
}

impl RpcIndexEngine {
    pub fn new(
        plan: RpcIndexPlan,
        context: CanonicalDecoderContext,
        capacity: IndexCapacity,
    ) -> Result<Self> {
        let scan_requests = plan
            .finalized_scan_requests()?
            .into_iter()
            .map(|request| (request.request_id, request))
            .collect();
        let subscription_requests = plan
            .subscription_requests()?
            .into_iter()
            .map(|request| (request.request_id, request))
            .collect();
        Ok(Self {
            index: CanonicalAccountIndex::new(plan, context, capacity)?,
            scan_requests,
            subscription_requests,
            completed_scans: BTreeSet::new(),
            finalized_scan_snapshots: BTreeMap::new(),
            exact_snapshot_requests: BTreeMap::new(),
            exact_snapshots: BTreeMap::new(),
            registered_requests: BTreeSet::new(),
            active_subscriptions: BTreeMap::new(),
            pending_accounts: BTreeMap::new(),
            pending_account_count: 0,
            pending_account_bytes: 0,
            pending_root: None,
            next_receive_sequence: 1,
        })
    }

    #[must_use]
    pub const fn index(&self) -> &CanonicalAccountIndex {
        &self.index
    }

    /// Transport-decoded finalized owner snapshot retained for semantic
    /// planners that need exact cross-owner follow-up reads.
    #[must_use]
    pub fn finalized_scan_snapshot(
        &self,
        release_key: &str,
    ) -> Option<&FinalizedAccountSnapshotV1> {
        self.finalized_scan_snapshots.get(release_key)
    }

    /// Register one opaque semantic-planner request with the transport engine.
    pub fn register_exact_snapshot_request(
        &mut self,
        request: FinalizedExactAccountSnapshotRequestV1,
    ) -> Result<()> {
        let request_id = request.request().request_id;
        if self.exact_snapshot_requests.contains_key(&request_id)
            || self.exact_snapshots.contains_key(&request_id)
            || self.scan_requests.contains_key(&request_id)
            || self.subscription_requests.contains_key(&request_id)
        {
            return Err(RpcIndexEngineError::DuplicateResponse);
        }
        self.exact_snapshot_requests.insert(request_id, request);
        Ok(())
    }

    #[must_use]
    pub fn exact_snapshot_requests(&self) -> Vec<&FinalizedExactAccountSnapshotRequestV1> {
        self.exact_snapshot_requests.values().collect()
    }

    #[must_use]
    pub fn exact_snapshot(&self, request_id: u64) -> Option<&FinalizedAccountSnapshotV1> {
        self.exact_snapshots.get(&request_id)
    }

    /// Start one autonomous current Direct action-8 material cycle from the
    /// retained finalized Clutch owner scan. The emitted exact request is
    /// registered atomically with this engine before transport sees it.
    pub fn plan_direct_action8_cycle(
        &mut self,
        collateral_catalog: &CurrentCollateralReleaseCatalogV1<'_>,
        manifest: &ExplicitOperatorReleaseManifest,
        builder: &ProtocolTransactionBuilder,
        request_id: u64,
    ) -> Result<Option<u64>> {
        let release = self
            .index
            .acquisition_plan()
            .releases
            .iter()
            .find(|release| {
                release.program_id == manifest.clutch.program_id
                    && release.program_data == manifest.clutch.program_data
                    && release.elf_sha256 == manifest.clutch.elf_sha256
                    && release.deployment_slot == manifest.clutch.deployment_slot
            })
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let snapshot = self
            .finalized_scan_snapshots
            .get(&release.key())
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let request = plan_direct_action8_context_snapshot_v2(
            self.index.acquisition_plan(),
            &self.index.acquisition_plan().releases,
            collateral_catalog,
            manifest,
            builder,
            snapshot,
            request_id,
        )?;
        let Some(request) = request else { return Ok(None); };
        let registered_id = request.request().request_id;
        self.register_exact_snapshot_request(request)?;
        Ok(Some(registered_id))
    }

    /// Join the retained discovery/exact receipts and exhaustively materialize
    /// the typed batch consumed by `OperatorJsonApi`.
    pub fn materialize_direct_action8_batch(
        &self,
        collateral_catalog: &CurrentCollateralReleaseCatalogV1<'_>,
        manifest: &ExplicitOperatorReleaseManifest,
        builder: &ProtocolTransactionBuilder,
        exact_request_id: Option<u64>,
    ) -> Result<DirectAction8OperatorBatchV2> {
        let release = self
            .index
            .acquisition_plan()
            .releases
            .iter()
            .find(|release| {
                release.program_id == manifest.clutch.program_id
                    && release.program_data == manifest.clutch.program_data
                    && release.elf_sha256 == manifest.clutch.elf_sha256
                    && release.deployment_slot == manifest.clutch.deployment_slot
            })
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let discovery = self
            .finalized_scan_snapshots
            .get(&release.key())
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let exact = exact_request_id
            .map(|request_id| {
                self.exact_snapshots
                    .get(&request_id)
                    .ok_or(RpcIndexEngineError::UnknownRequest)
            })
            .transpose()?;
        let snapshot = join_direct_action8_finalized_snapshots_v2(discovery, exact)?;
        enumerate_direct_action8_material_v2(
            &self.index.acquisition_plan().releases,
            collateral_catalog,
            manifest,
            builder,
            &snapshot,
        )
        .map_err(Into::into)
    }

    /// Start one exhaustive Dealer action-25 target-8/9 acquisition cycle.
    /// The retained owner scan selects every Retiring facility; callers cannot
    /// supply a facility, target discriminator, or semantic identifier.
    pub fn plan_dealer_terminal_cycle(
        &mut self,
        collateral: &AuthenticatedCurrentCollateralReleaseV1<'_>,
        builder: &ProtocolTransactionBuilder,
        lookup_table: Address,
        request_id: u64,
    ) -> Result<Option<u64>> {
        let release = self
            .index
            .acquisition_plan()
            .releases
            .iter()
            .find(|release| {
                release.program_id == builder.clutch_program()
                    && release.elf_sha256 == builder.clutch_release_sha256()
            })
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let snapshot = self
            .finalized_scan_snapshots
            .get(&release.key())
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let request = plan_dealer_terminal_snapshot_v1(
            self.index.acquisition_plan(),
            release,
            collateral,
            builder,
            snapshot,
            lookup_table,
            request_id,
        )?;
        let Some(request) = request else { return Ok(None); };
        let registered_id = request.request().request_id;
        self.register_exact_snapshot_request(request)?;
        Ok(Some(registered_id))
    }

    /// Join the exact same-slot reread and materialize the complete target-8/9
    /// batch consumed by capability discovery and the operator API.
    pub fn materialize_dealer_terminal_batch(
        &self,
        collateral: AuthenticatedCurrentCollateralReleaseV1<'_>,
        builder: &ProtocolTransactionBuilder,
        lookup_table: Address,
        exact_request_id: u64,
    ) -> Result<DealerTerminalOperatorBatchV1> {
        let release = self
            .index
            .acquisition_plan()
            .releases
            .iter()
            .find(|release| {
                release.program_id == builder.clutch_program()
                    && release.elf_sha256 == builder.clutch_release_sha256()
            })
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let discovery = self
            .finalized_scan_snapshots
            .get(&release.key())
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let exact = self
            .exact_snapshots
            .get(&exact_request_id)
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let snapshot = join_dealer_terminal_snapshots_v1(discovery, exact)?;
        enumerate_dealer_terminal_material_v1(
            release,
            collateral,
            builder,
            &snapshot,
            lookup_table,
        )
        .map_err(Into::into)
    }

    #[must_use]
    pub fn bootstrap_requests(&self) -> Vec<&PlannedRpcRequest> {
        self.scan_requests
            .iter()
            .filter_map(|(request_id, request)| {
                (!self.completed_scans.contains(request_id)).then_some(request)
            })
            .collect()
    }

    #[must_use]
    pub fn unregistered_subscription_requests(&self) -> Vec<&PlannedRpcRequest> {
        self.subscription_requests
            .iter()
            .filter_map(|(request_id, request)| {
                (!self.registered_requests.contains(request_id)).then_some(request)
            })
            .collect()
    }

    #[must_use]
    pub fn bootstrap_complete(&self) -> bool {
        self.completed_scans.len() == self.scan_requests.len()
    }

    pub fn begin_finalized_rescan(&mut self) -> Result<()> {
        if !self.bootstrap_complete() {
            return Err(RpcIndexEngineError::DuplicateResponse);
        }
        self.completed_scans.clear();
        self.finalized_scan_snapshots.clear();
        self.exact_snapshot_requests.clear();
        self.exact_snapshots.clear();
        Ok(())
    }

    /// Withdraw the complete processed generation before any reconnect.
    /// Server-assigned subscription IDs are connection-scoped and therefore
    /// can never survive transport loss. Finalized scan state remains intact.
    pub fn begin_processed_reconnect(&mut self) -> ProcessedReconnectRollback {
        let rollback = ProcessedReconnectRollback {
            indexed_versions_removed: self.index.rollback_processed_transport(),
            buffered_accounts_removed: self.pending_account_count,
        };
        self.registered_requests.clear();
        self.active_subscriptions.clear();
        self.pending_accounts.clear();
        self.pending_account_count = 0;
        self.pending_account_bytes = 0;
        self.pending_root = None;
        rollback
    }

    #[must_use]
    pub fn status(&self) -> RpcIndexEngineStatus {
        RpcIndexEngineStatus {
            bootstrap_complete: self.bootstrap_complete(),
            remaining_scans: self.scan_requests.len() - self.completed_scans.len(),
            remaining_subscription_registrations: self.subscription_requests.len()
                - self.registered_requests.len(),
            active_subscriptions: self.active_subscriptions.len(),
            pending_accounts: self.pending_account_count,
            pending_account_bytes: self.pending_account_bytes,
            pending_root: self.pending_root,
            next_receive_sequence: self.next_receive_sequence,
        }
    }

    pub fn admit_scan_response(
        &mut self,
        request_id: u64,
        response: &Value,
    ) -> Result<RpcIndexEngineEvent> {
        let request = self
            .scan_requests
            .get(&request_id)
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        if self.completed_scans.contains(&request_id) {
            return Err(RpcIndexEngineError::DuplicateResponse);
        }
        let result = decode_response_result(self.index.acquisition_plan(), request, response)?;
        let scan_slot = program_scan_context_slot(result)?;
        let snapshot = decode_program_scan_snapshot_v1(
            self.index.acquisition_plan(),
            request,
            result,
            self.next_receive_sequence,
        )?;
        let accounts = snapshot.accounts();
        let account_count = accounts.len();
        let sequence_advance = u64::try_from(accounts.len().max(1))
            .map_err(|_| RpcIndexEngineError::SequenceExhausted)?;
        let next_sequence = self
            .next_receive_sequence
            .checked_add(sequence_advance)
            .ok_or(RpcIndexEngineError::SequenceExhausted)?;
        let mut next_index = self.index.clone();
        let seen: BTreeSet<Address> = accounts.iter().map(|account| account.address).collect();
        for account in accounts.iter().cloned() {
            next_index.ingest(account)?;
        }
        next_index.reconcile_finalized_scan(
            &request.release_key,
            scan_slot,
            next_sequence - 1,
            &seen,
        )?;
        self.index = next_index;
        self.finalized_scan_snapshots
            .insert(request.release_key.clone(), snapshot);
        self.next_receive_sequence = next_sequence;
        self.completed_scans.insert(request_id);
        Ok(RpcIndexEngineEvent::FinalizedScanAdmitted {
            release_key: request.release_key.clone(),
            account_count,
        })
    }

    pub fn admit_subscription_response(
        &mut self,
        request_id: u64,
        response: &Value,
    ) -> Result<RpcIndexEngineEvent> {
        let request = self
            .subscription_requests
            .get(&request_id)
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        if self.registered_requests.contains(&request_id) {
            return Err(RpcIndexEngineError::DuplicateResponse);
        }
        let subscription_id =
            decode_subscription_registration(self.index.acquisition_plan(), request, response)?;
        if self.active_subscriptions.contains_key(&subscription_id) {
            return Err(RpcIndexEngineError::DuplicateSubscription);
        }
        self.active_subscriptions
            .insert(subscription_id, request.clone());
        self.registered_requests.insert(request_id);
        Ok(RpcIndexEngineEvent::SubscriptionBound {
            request_id,
            subscription_id,
        })
    }

    pub fn admit_exact_snapshot_response(
        &mut self,
        request_id: u64,
        response: &Value,
    ) -> Result<RpcIndexEngineEvent> {
        let request = self
            .exact_snapshot_requests
            .get(&request_id)
            .ok_or(RpcIndexEngineError::UnknownRequest)?;
        let result = decode_response_result(
            self.index.acquisition_plan(),
            request.request(),
            response,
        )?;
        let snapshot = decode_finalized_exact_account_snapshot_v1(
            self.index.acquisition_plan(),
            request,
            result,
            self.next_receive_sequence,
        )?;
        let account_count = snapshot.accounts().len();
        let sequence_advance = u64::try_from(account_count.max(1))
            .map_err(|_| RpcIndexEngineError::SequenceExhausted)?;
        self.next_receive_sequence = self
            .next_receive_sequence
            .checked_add(sequence_advance)
            .ok_or(RpcIndexEngineError::SequenceExhausted)?;
        let slot = snapshot.receipt().slot();
        self.exact_snapshots.insert(request_id, snapshot);
        self.exact_snapshot_requests.remove(&request_id);
        Ok(RpcIndexEngineEvent::FinalizedExactSnapshotAdmitted {
            request_id,
            account_count,
            slot,
        })
    }

    pub fn admit_notification(&mut self, notification: &Value) -> Result<Vec<RpcIndexEngineEvent>> {
        let subscription_id = notification_subscription_id(notification)?;
        let request = self
            .active_subscriptions
            .get(&subscription_id)
            .cloned()
            .ok_or(RpcIndexEngineError::UnknownSubscription)?;
        let receive_sequence = self.next_receive_sequence;
        let next_sequence = receive_sequence
            .checked_add(1)
            .ok_or(RpcIndexEngineError::SequenceExhausted)?;
        let events = match request.purpose {
            RpcRequestPurpose::ProgramSubscription => {
                let update = decode_program_notification(
                    self.index.acquisition_plan(),
                    &request,
                    notification,
                    receive_sequence,
                )?;
                self.admit_processed_update(update)?
            }
            RpcRequestPurpose::BlockSubscription => {
                let slot = decode_block_notification(
                    self.index.acquisition_plan(),
                    &request,
                    notification,
                    receive_sequence,
                )?;
                let slot_number = slot.slot;
                self.index.observe_slot(slot)?;
                let mut events = vec![RpcIndexEngineEvent::SlotObserved { slot: slot_number }];
                if self.index.forks().is_frozen(slot_number) {
                    events.extend(self.drain_slot(slot_number)?);
                }
                events.extend(self.try_finalize_pending_root()?);
                events
            }
            RpcRequestPurpose::SlotSubscription => {
                let update = decode_slot_update_notification(
                    self.index.acquisition_plan(),
                    &request,
                    notification,
                    receive_sequence,
                )?;
                let slot = update.slot;
                let kind = update.kind;
                self.index.observe_slot_update(update)?;
                let mut events = vec![RpcIndexEngineEvent::SlotUpdated { slot, kind }];
                match kind {
                    ObservedSlotUpdateKind::Frozen => events.extend(self.drain_slot(slot)?),
                    ObservedSlotUpdateKind::Dead => {
                        let dead_pending_slots = self
                            .pending_accounts
                            .keys()
                            .copied()
                            .filter(|pending_slot| {
                                *pending_slot == slot
                                    || self.index.forks().slot_is_on_dead_branch(*pending_slot)
                            })
                            .collect::<Vec<_>>();
                        for pending_slot in dead_pending_slots {
                            let Some(accounts) = self.pending_accounts.remove(&pending_slot) else {
                                continue;
                            };
                            let bytes = pending_bytes(&accounts)?;
                            self.pending_account_count = self
                                .pending_account_count
                                .checked_sub(accounts.len())
                                .ok_or(RpcIndexEngineError::CapacityExceeded)?;
                            self.pending_account_bytes = self
                                .pending_account_bytes
                                .checked_sub(bytes)
                                .ok_or(RpcIndexEngineError::CapacityExceeded)?;
                            events.push(RpcIndexEngineEvent::BufferedAccountsDropped {
                                slot: pending_slot,
                                account_count: accounts.len(),
                            });
                        }
                        let account_count = self.index.rollback_dead_processed_versions();
                        if account_count > 0 {
                            events.push(RpcIndexEngineEvent::IndexedAccountsRolledBack {
                                slot,
                                account_count,
                            });
                        }
                    }
                    _ => {}
                }
                events.extend(self.try_finalize_pending_root()?);
                events
            }
            RpcRequestPurpose::RootSubscription => {
                let root = decode_root_notification(
                    self.index.acquisition_plan(),
                    &request,
                    notification,
                )?;
                self.pending_root = Some(self.pending_root.map_or(root, |prior| prior.max(root)));
                let events = self.try_finalize_pending_root()?;
                if events.is_empty() {
                    vec![RpcIndexEngineEvent::RootDeferred { slot: root }]
                } else {
                    events
                }
            }
            RpcRequestPurpose::ProgramScan | RpcRequestPurpose::ExactAccountSnapshot => {
                return Err(RpcIndexEngineError::UnknownRequest)
            }
        };
        self.next_receive_sequence = next_sequence;
        Ok(events)
    }

    fn admit_processed_update(
        &mut self,
        update: ObservedRpcProgramUpdate,
    ) -> Result<Vec<RpcIndexEngineEvent>> {
        let address = update.address();
        let slot = update.slot();
        if self.index.forks().is_dead(slot) || self.index.forks().slot_is_on_dead_branch(slot) {
            return Ok(vec![RpcIndexEngineEvent::BufferedAccountsDropped {
                slot,
                account_count: 1,
            }]);
        }
        if self.index.forks().is_frozen(slot) && self.index.forks().unique_hash_at(slot).is_ok() {
            return match update {
                ObservedRpcProgramUpdate::Present(account) => {
                    self.index.ingest(account)?;
                    Ok(vec![RpcIndexEngineEvent::AccountIndexed { address, slot }])
                }
                ObservedRpcProgramUpdate::Removed(removal) => {
                    let observed_owner = removal.observed_owner;
                    let kind = removal.kind;
                    let projection_withdrawn = self.index.record_processed_removal(removal)?;
                    Ok(vec![RpcIndexEngineEvent::AccountRemoved {
                        address,
                        observed_owner,
                        slot,
                        kind,
                        projection_withdrawn,
                    }])
                }
            };
        }
        let maximum_pending = self
            .index
            .acquisition_plan()
            .bounds
            .maximum_accounts_per_scan;
        if self.pending_account_count >= maximum_pending {
            return Err(RpcIndexEngineError::CapacityExceeded);
        }
        let next_pending_bytes = self
            .pending_account_bytes
            .checked_add(update.retained_data_bytes())
            .ok_or(RpcIndexEngineError::CapacityExceeded)?;
        if next_pending_bytes
            > self
                .index
                .acquisition_plan()
                .bounds
                .maximum_total_response_bytes
        {
            return Err(RpcIndexEngineError::CapacityExceeded);
        }
        let event = match &update {
            ObservedRpcProgramUpdate::Present(_) => {
                RpcIndexEngineEvent::AccountBuffered { address, slot }
            }
            ObservedRpcProgramUpdate::Removed(removal) => {
                RpcIndexEngineEvent::AccountRemovalBuffered {
                    address,
                    slot,
                    kind: removal.kind,
                }
            }
        };
        self.pending_accounts.entry(slot).or_default().push(update);
        self.pending_account_count = self
            .pending_account_count
            .checked_add(1)
            .ok_or(RpcIndexEngineError::CapacityExceeded)?;
        self.pending_account_bytes = next_pending_bytes;
        Ok(vec![event])
    }

    fn drain_slot(&mut self, slot: u64) -> Result<Vec<RpcIndexEngineEvent>> {
        let Some(accounts) = self.pending_accounts.get(&slot) else {
            return Ok(Vec::new());
        };
        self.index.forks().unique_hash_at(slot)?;
        if self.index.forks().slot_is_on_dead_branch(slot) {
            let account_count = accounts.len();
            let account_bytes = pending_bytes(accounts)?;
            self.pending_accounts.remove(&slot);
            self.pending_account_count = self
                .pending_account_count
                .checked_sub(account_count)
                .ok_or(RpcIndexEngineError::CapacityExceeded)?;
            self.pending_account_bytes = self
                .pending_account_bytes
                .checked_sub(account_bytes)
                .ok_or(RpcIndexEngineError::CapacityExceeded)?;
            return Ok(vec![RpcIndexEngineEvent::BufferedAccountsDropped {
                slot,
                account_count,
            }]);
        }
        let mut next_index = self.index.clone();
        let mut events = Vec::with_capacity(accounts.len());
        for update in accounts.iter().cloned() {
            let address = update.address();
            match update {
                ObservedRpcProgramUpdate::Present(account) => {
                    next_index.ingest(account)?;
                    events.push(RpcIndexEngineEvent::AccountIndexed { address, slot });
                }
                ObservedRpcProgramUpdate::Removed(removal) => {
                    let observed_owner = removal.observed_owner;
                    let kind = removal.kind;
                    let projection_withdrawn = next_index.record_processed_removal(removal)?;
                    events.push(RpcIndexEngineEvent::AccountRemoved {
                        address,
                        observed_owner,
                        slot,
                        kind,
                        projection_withdrawn,
                    });
                }
            }
        }
        let account_count = accounts.len();
        let account_bytes = pending_bytes(accounts)?;
        self.index = next_index;
        self.pending_accounts.remove(&slot);
        self.pending_account_count = self
            .pending_account_count
            .checked_sub(account_count)
            .ok_or(RpcIndexEngineError::CapacityExceeded)?;
        self.pending_account_bytes = self
            .pending_account_bytes
            .checked_sub(account_bytes)
            .ok_or(RpcIndexEngineError::CapacityExceeded)?;
        Ok(events)
    }

    fn try_finalize_pending_root(&mut self) -> Result<Vec<RpcIndexEngineEvent>> {
        let Some(root) = self.pending_root else {
            return Ok(Vec::new());
        };
        match self.index.forks().unique_hash_at(root) {
            Ok(_) => {
                self.index.finalize_root(root)?;
                let mut events = self.drain_slot(root)?;
                self.pending_root = None;
                events.push(RpcIndexEngineEvent::RootFinalized { slot: root });
                Ok(events)
            }
            Err(AccountIndexError::UnknownFork) => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }
}

fn pending_bytes(accounts: &[ObservedRpcProgramUpdate]) -> Result<usize> {
    accounts.iter().try_fold(0usize, |total, account| {
        total
            .checked_add(account.retained_data_bytes())
            .ok_or(RpcIndexEngineError::CapacityExceeded)
    })
}
