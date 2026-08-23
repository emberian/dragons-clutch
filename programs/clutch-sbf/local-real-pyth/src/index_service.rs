//! Resumable orchestration for the transport-neutral RPC/index plane.
//!
//! The engine emits exact JSON-RPC requests and accepts bounded decoded
//! responses. It never opens a socket. Program-account notifications remain
//! buffered until the corresponding slot is both block-identified and frozen,
//! because a slot number alone is not a fork identity.

use crate::account_index::{
    AccountIndexError, CanonicalAccountIndex, CanonicalDecoderContext, IndexCapacity,
};
use crate::rpc_index::{
    decode_block_notification, decode_program_notification, decode_program_scan_result,
    decode_response_result, decode_root_notification, decode_slot_update_notification,
    decode_subscription_registration, notification_subscription_id, program_scan_context_slot,
    ObservedRpcAccount, ObservedSlotUpdateKind, PlannedRpcRequest, RpcIndexError, RpcIndexPlan,
    RpcRequestPurpose,
};
use serde_json::Value;
use solana_address::Address;
use std::collections::{BTreeMap, BTreeSet};

pub type Result<T> = core::result::Result<T, RpcIndexEngineError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RpcIndexEngineError {
    Rpc(RpcIndexError),
    Account(AccountIndexError),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RpcIndexEngineEvent {
    FinalizedScanAdmitted {
        release_key: String,
        account_count: usize,
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
    BufferedAccountsDropped {
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
    registered_requests: BTreeSet<u64>,
    active_subscriptions: BTreeMap<u64, PlannedRpcRequest>,
    pending_accounts: BTreeMap<u64, Vec<ObservedRpcAccount>>,
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
        Ok(())
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
        let accounts = decode_program_scan_result(
            self.index.acquisition_plan(),
            request,
            result,
            self.next_receive_sequence,
        )?;
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
        self.next_receive_sequence = next_sequence;
        self.completed_scans.insert(request_id);
        Ok(RpcIndexEngineEvent::FinalizedScanAdmitted {
            release_key: request.release_key.clone(),
            account_count: accounts.len(),
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
                let account = decode_program_notification(
                    self.index.acquisition_plan(),
                    &request,
                    notification,
                    receive_sequence,
                )?;
                self.admit_processed_account(account)?
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
                        if let Some(accounts) = self.pending_accounts.remove(&slot) {
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
                                slot,
                                account_count: accounts.len(),
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
            RpcRequestPurpose::ProgramScan => return Err(RpcIndexEngineError::UnknownRequest),
        };
        self.next_receive_sequence = next_sequence;
        Ok(events)
    }

    fn admit_processed_account(
        &mut self,
        account: ObservedRpcAccount,
    ) -> Result<Vec<RpcIndexEngineEvent>> {
        let address = account.address;
        let slot = account.provenance.slot;
        if self.index.forks().is_dead(slot) {
            return Ok(vec![RpcIndexEngineEvent::BufferedAccountsDropped {
                slot,
                account_count: 1,
            }]);
        }
        if self.index.forks().is_frozen(slot) {
            self.index.ingest(account)?;
            return Ok(vec![RpcIndexEngineEvent::AccountIndexed { address, slot }]);
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
            .checked_add(account.data.len())
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
        self.pending_accounts.entry(slot).or_default().push(account);
        self.pending_account_count = self
            .pending_account_count
            .checked_add(1)
            .ok_or(RpcIndexEngineError::CapacityExceeded)?;
        self.pending_account_bytes = next_pending_bytes;
        Ok(vec![RpcIndexEngineEvent::AccountBuffered { address, slot }])
    }

    fn drain_slot(&mut self, slot: u64) -> Result<Vec<RpcIndexEngineEvent>> {
        let Some(accounts) = self.pending_accounts.get(&slot) else {
            return Ok(Vec::new());
        };
        self.index.forks().unique_hash_at(slot)?;
        let mut next_index = self.index.clone();
        let mut events = Vec::with_capacity(accounts.len());
        for account in accounts.iter().cloned() {
            let address = account.address;
            next_index.ingest(account)?;
            events.push(RpcIndexEngineEvent::AccountIndexed { address, slot });
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
                self.pending_root = None;
                Ok(vec![RpcIndexEngineEvent::RootFinalized { slot: root }])
            }
            Err(AccountIndexError::UnknownFork) => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }
}

fn pending_bytes(accounts: &[ObservedRpcAccount]) -> Result<usize> {
    accounts.iter().try_fold(0usize, |total, account| {
        total
            .checked_add(account.data.len())
            .ok_or(RpcIndexEngineError::CapacityExceeded)
    })
}
