//! The observation loop for one watched account set.
//!
//! §4.11, in order:
//!
//! 1. One `getMultipleAccounts` at `commitment: finalized` covering the entire
//!    set.  Per-account `getAccountInfo` is **forbidden** for an observation:
//!    the batch call returns a single `context.slot` for the whole response,
//!    and mixed-slot account sets are the observation bug this family most
//!    needs to not have.
//! 2. A Loader V3 `ProgramData` position carries a 45-byte inline prefix; its
//!    tail digest covers ~2.3 MB, so it is cached under
//!    `(programdata_pubkey, deployment_slot)` and recomputed only when
//!    `deployment_slot` — which sits in the prefix the batch already fetched —
//!    changes.  A redeploy is always detected from the prefix, so a stale
//!    cached digest is unreachable.
//! 3. Build each body, verify `inline_len` against the pinned width, sign one
//!    attestation per account and one seal per set.
//!
//! **Failure behaviour, and it is a rule rather than a policy.** On RPC
//! disagreement, a missing account, a `data_len` outside the configured
//! admitted set, or a `deployment_slot` change, the daemon **stops attesting
//! that set** and emits a diagnostic.  It never attests a partial or repaired
//! observation.  The market's own funded failure path (§4.8) is the correct
//! handling of a stopped relayer, and it is better than any repair this daemon
//! could invent: a repaired observation is a claim about mainnet that mainnet
//! does not support, and no one downstream can tell it apart from a true one.
//!
//! A transport failure is deliberately *not* on that list.  A refused
//! connection is not a disagreement about what mainnet holds; it is the absence
//! of an answer, and the correct response is to try again next cycle.  Only a
//! cluster that answered, and answered something inadmissible, stops the set.

use std::collections::HashMap;

use dclutch_relay_contract::wire::{
    AccountObservationV1, AttestationMessageV1, ObservationSetSealV1,
};
use dclutch_relay_contract::{RELAYED_SEAL_BYTES, SHA256_EMPTY_DIGEST};

use crate::chain::{is_loader_v3_programdata, programdata_deployment_slot};
use crate::config::{AccountSetConfig, PositionConfig};
use crate::derive::{SetDigestFold, TailHasher};
use crate::error::{RelayerError, Result};
use crate::id32::{ID_BYTES, base58};
use crate::keys::AttestationSigner;
use crate::rpc::{BatchRead, ObservedAccount, RpcClient};

/// Hard ceiling on paged reads for one account body.
const MAX_BODY_PAGES: u32 = 1024;

/// Where one position's tail digest came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TailDigestSource {
    /// The body carries the account in full; the digest is over the empty tail.
    FullyInline,
    /// The tail was paged and hashed this cycle.
    Paged {
        /// How many `getAccountInfo` pages were read.
        pages: u32,
        /// How many tail bytes were hashed.
        bytes: u64,
    },
    /// The tail digest was cached under `(pubkey, deployment_slot)`.
    Cached {
        /// The deployment slot the cache entry is keyed by.
        deployment_slot: u64,
    },
}

impl TailDigestSource {
    /// The stable string an artifact records this as.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FullyInline => "fully-inline",
            Self::Paged { .. } => "paged",
            Self::Cached { .. } => "cached",
        }
    }
}

/// One attested position of one cycle.
#[derive(Clone, Debug)]
pub struct ObservedPosition {
    /// Position in the canonical ordered set.
    pub set_index: u16,
    /// The pinned account address.
    pub key: [u8; ID_BYTES],
    /// The owner as read.
    pub owner: [u8; ID_BYTES],
    /// Lamports as read.
    pub lamports: u64,
    /// Complete on-chain data length as read.
    pub data_len: u32,
    /// The release-pinned inline prefix.
    pub inline: Vec<u8>,
    /// Executable flag as read.
    pub executable: bool,
    /// SHA-256 over `data[inline_len..data_len]`.
    pub tail_digest: [u8; ID_BYTES],
    /// How that digest was obtained.
    pub tail_digest_source: TailDigestSource,
    /// The exact encoded observation body, which is what the fold absorbs.
    pub body_bytes: Vec<u8>,
    /// The exact encoded attestation message, which is what was signed.
    pub message_bytes: Vec<u8>,
    /// The signature over `message_bytes`.
    pub signature: [u8; 64],
}

/// One completed cycle for one set.
#[derive(Clone, Debug)]
pub struct ObservationCycle {
    /// Operator-chosen set name.
    pub set_name: String,
    /// The founding-time pinned set identity.
    pub account_set_id: [u8; ID_BYTES],
    /// Genesis hash of the cluster read.
    pub observed_cluster_id: [u8; ID_BYTES],
    /// `ProviderReleaseV1.provider_family_id`.
    pub relay_family_id: [u8; ID_BYTES],
    /// `ProviderReleaseV1.decoding_rules_id`.
    pub decoding_rules_id: [u8; ID_BYTES],
    /// The single finalized slot the whole set was read at.
    pub observed_slot: u64,
    /// Set cardinality.
    pub set_count: u16,
    /// The running fold over the accepted bodies.
    pub set_digest: [u8; ID_BYTES],
    /// One entry per position, in set order.
    pub positions: Vec<ObservedPosition>,
    /// The exact 156-byte seal message.
    pub seal_bytes: [u8; RELAYED_SEAL_BYTES],
    /// The signature over the seal message.
    pub seal_signature: [u8; 64],
    /// The signer's public key.
    pub signer: [u8; ID_BYTES],
    /// The verbatim primary `getMultipleAccounts` response.
    pub raw_batch: serde_json::Value,
    /// Verbatim cross-check responses, by endpoint host.
    pub cross_check_raw: Vec<(String, serde_json::Value)>,
    /// Host of the endpoint the observation was taken from.
    pub primary_endpoint_host: String,
    /// How many paged body reads this cycle performed.
    pub paged_reads: u32,
}

/// One watched set's state across cycles.
pub struct SetWatcher {
    config: AccountSetConfig,
    observed_cluster_id: [u8; ID_BYTES],
    body_page_bytes: usize,
    tail_cache: HashMap<([u8; ID_BYTES], u64), [u8; ID_BYTES]>,
    deployment_slots: HashMap<[u8; ID_BYTES], u64>,
    stopped: Option<String>,
}

impl SetWatcher {
    /// Start watching one configured set.
    pub fn new(
        config: AccountSetConfig,
        observed_cluster_id: [u8; ID_BYTES],
        body_page_bytes: usize,
    ) -> Self {
        Self {
            config,
            observed_cluster_id,
            body_page_bytes,
            tail_cache: HashMap::new(),
            deployment_slots: HashMap::new(),
            stopped: None,
        }
    }

    /// The set this watcher observes.
    pub fn config(&self) -> &AccountSetConfig {
        &self.config
    }

    /// Why this set stopped being attested, if it has.
    pub fn stopped_reason(&self) -> Option<&str> {
        self.stopped.as_deref()
    }

    /// Record a `deployment_slot` seen before this process started, so a
    /// restart does not silently forget an upgrade it had already refused.
    pub fn seed_deployment_slot(&mut self, key: [u8; ID_BYTES], slot: u64) {
        self.deployment_slots.insert(key, slot);
    }

    fn refuse(&mut self, reason: impl Into<String>) -> RelayerError {
        let reason = reason.into();
        self.stopped = Some(reason.clone());
        RelayerError::ObservationRefused {
            set: self.config.name.clone(),
            reason,
        }
    }

    /// Take one observation and sign it.
    ///
    /// Returns [`RelayerError::ObservationRefused`] once the set has stopped,
    /// and stays stopped: the whole point of the rule is that it is not
    /// recoverable inside the daemon.
    pub async fn observe(
        &mut self,
        primary: &RpcClient,
        cross_check: &[RpcClient],
        signer: &AttestationSigner,
    ) -> Result<ObservationCycle> {
        if let Some(reason) = &self.stopped {
            return Err(RelayerError::ObservationRefused {
                set: self.config.name.clone(),
                reason: reason.clone(),
            });
        }

        let set_count = self.config.set_count()?;
        let keys: Vec<[u8; ID_BYTES]> = self
            .config
            .positions
            .iter()
            .map(|position| position.key)
            .collect();
        let slice_len = self.config.max_inline_len();

        // One call, one slot, the whole set.
        let batch = primary
            .get_multiple_accounts(&keys, slice_len, None)
            .await?;
        let observed_slot = batch.slot;

        let cross_check_raw = self
            .cross_check(cross_check, &keys, slice_len, &batch)
            .await?;

        let mut fold = SetDigestFold::seed(self.config.account_set_id, observed_slot)?;
        let mut positions = Vec::with_capacity(self.config.positions.len());
        let mut paged_reads = 0u32;

        let pinned: Vec<PositionConfig> = self.config.positions.clone();
        for (index, position) in pinned.into_iter().enumerate() {
            let set_index = u16::try_from(index)
                .map_err(|_| RelayerError::config("set index did not fit in a u16"))?;
            let account = match batch.accounts.get(index) {
                Some(Some(account)) => account.clone(),
                Some(None) => {
                    return Err(self.refuse(format!(
                        "position {set_index} ({}) does not exist on the observed cluster at slot \
                         {observed_slot}",
                        base58(&position.key)
                    )));
                }
                None => {
                    return Err(self.refuse(
                        "the batch response held fewer accounts than the set has positions",
                    ));
                }
            };

            let prepared = self
                .prepare_position(
                    primary,
                    set_index,
                    &position,
                    &account,
                    slice_len,
                    observed_slot,
                )
                .await?;
            paged_reads = paged_reads.saturating_add(prepared.pages);

            let body = AccountObservationV1::new(
                position.key,
                account.owner,
                account.lamports,
                prepared.data_len,
                &prepared.inline,
                account.executable,
                prepared.tail_digest,
            )
            .map_err(|error| {
                RelayerError::wire(&format!("observation body at position {set_index}"), error)
            })?;

            // The pinned width is checked against the body that was actually
            // built, not against the config that produced it, so a truncation
            // anywhere upstream is caught here.
            body.require_pinned_position(
                position.key,
                position.expected_owner,
                position.inline_len,
            )
            .map_err(|error| {
                RelayerError::wire(
                    &format!("pinned position check at position {set_index}"),
                    error,
                )
            })?;

            let mut body_bytes = vec![0u8; body.encoded_len()];
            body.encode_into(&mut body_bytes).map_err(|error| {
                RelayerError::wire(&format!("encode body at position {set_index}"), error)
            })?;
            fold.absorb(&body_bytes);

            let message = AttestationMessageV1::new(
                self.observed_cluster_id,
                self.config.relay_family_id,
                self.config.decoding_rules_id,
                self.config.account_set_id,
                observed_slot,
                set_index,
                set_count,
                body,
            )
            .map_err(|error| {
                RelayerError::wire(&format!("attestation at position {set_index}"), error)
            })?;
            let mut message_bytes = vec![0u8; message.encoded_len()];
            message.encode_into(&mut message_bytes).map_err(|error| {
                RelayerError::wire(
                    &format!("encode attestation at position {set_index}"),
                    error,
                )
            })?;

            // Sign exactly the encoded bytes, and check the signature verifies
            // before it is ever published.  A published attestation that does
            // not verify is worse than none: it is a falsifiable claim that
            // falsifies for the wrong reason.
            let signature = signer.sign(&message_bytes);
            if !signer.verify(&message_bytes, &signature) {
                return Err(
                    self.refuse("the attestation signature did not verify against its own message")
                );
            }

            positions.push(ObservedPosition {
                set_index,
                key: position.key,
                owner: account.owner,
                lamports: account.lamports,
                data_len: prepared.data_len,
                inline: prepared.inline,
                executable: account.executable,
                tail_digest: prepared.tail_digest,
                tail_digest_source: prepared.source,
                body_bytes,
                message_bytes,
                signature,
            });
        }

        let set_digest = fold.digest();
        let seal = ObservationSetSealV1::new(
            self.observed_cluster_id,
            self.config.relay_family_id,
            self.config.account_set_id,
            observed_slot,
            set_count,
            set_digest,
        )
        .map_err(|error| RelayerError::wire("seal message", error))?;
        let seal_bytes = seal
            .to_bytes()
            .map_err(|error| RelayerError::wire("encode seal", error))?;
        let seal_signature = signer.sign(&seal_bytes);
        if !signer.verify(&seal_bytes, &seal_signature) {
            return Err(self.refuse("the seal signature did not verify against its own message"));
        }

        Ok(ObservationCycle {
            set_name: self.config.name.clone(),
            account_set_id: self.config.account_set_id,
            observed_cluster_id: self.observed_cluster_id,
            relay_family_id: self.config.relay_family_id,
            decoding_rules_id: self.config.decoding_rules_id,
            observed_slot,
            set_count,
            set_digest,
            positions,
            seal_bytes,
            seal_signature,
            signer: signer.public_key(),
            raw_batch: batch.raw,
            cross_check_raw,
            primary_endpoint_host: primary.host().to_owned(),
            paged_reads,
        })
    }

    async fn cross_check(
        &mut self,
        cross_check: &[RpcClient],
        keys: &[[u8; ID_BYTES]],
        slice_len: u16,
        primary: &BatchRead,
    ) -> Result<Vec<(String, serde_json::Value)>> {
        let mut raws = Vec::with_capacity(cross_check.len());
        for endpoint in cross_check {
            // `minContextSlot` makes the comparison meaningful: without it a
            // lagging endpoint would "disagree" merely by being behind.
            let other = endpoint
                .get_multiple_accounts(keys, slice_len, Some(primary.slot))
                .await?;
            let same_slot = other.slot == primary.slot;
            for (index, key) in keys.iter().enumerate() {
                let mine = primary.accounts.get(index).and_then(Option::as_ref);
                let theirs = other.accounts.get(index).and_then(Option::as_ref);
                if let Some(reason) = disagreement(mine, theirs, same_slot).map(|reason| {
                    format!(
                        "endpoint {} disagrees about position {index} ({}): {reason}",
                        endpoint.host(),
                        base58(key)
                    )
                }) {
                    return Err(self.refuse(reason));
                }
            }
            raws.push((endpoint.host().to_owned(), other.raw));
        }
        Ok(raws)
    }

    async fn prepare_position(
        &mut self,
        rpc: &RpcClient,
        set_index: u16,
        position: &PositionConfig,
        account: &ObservedAccount,
        slice_len: u16,
        observed_slot: u64,
    ) -> Result<PreparedPosition> {
        if account.owner != position.expected_owner {
            return Err(self.refuse(format!(
                "position {set_index} ({}) is owned by {} but the set pins {}",
                base58(&position.key),
                base58(&account.owner),
                base58(&position.expected_owner)
            )));
        }

        let Ok(data_len) = u32::try_from(account.data_len) else {
            return Err(self.refuse(format!(
                "position {set_index} reports a data_len of {} which does not fit the wire's u32",
                account.data_len
            )));
        };
        if !position.admits_data_len(data_len) {
            return Err(self.refuse(format!(
                "position {set_index} ({}) has data_len {data_len}, which is outside the admitted \
                 set {:?} (pinned inline_len {})",
                base58(&position.key),
                position.admitted_data_lens,
                position.inline_len
            )));
        }

        // One dataSlice covers the whole batch, so a position gets
        // min(slice_len, data_len) bytes back and is then truncated to its own
        // pinned width.  A different count means the endpoint answered a
        // different question than was asked.
        let expected_returned = u64::from(slice_len).min(account.data_len);
        if u64::try_from(account.data.len()).unwrap_or(u64::MAX) != expected_returned {
            return Err(self.refuse(format!(
                "position {set_index} returned {} sliced bytes where {expected_returned} were \
                 asked for",
                account.data.len()
            )));
        }
        let inline_len = usize::from(position.inline_len);
        let inline = account
            .data
            .get(..inline_len)
            .ok_or_else(|| RelayerError::ObservationRefused {
                set: self.config.name.clone(),
                reason: format!(
                    "position {set_index} returned fewer bytes than its pinned inline_len \
                         {inline_len}"
                ),
            })?
            .to_vec();

        let programdata_slot =
            if is_loader_v3_programdata(&position.expected_owner, position.inline_len) {
                let Some(slot) = programdata_deployment_slot(&inline) else {
                    return Err(self.refuse(format!(
                        "position {set_index} ({}) is pinned as a Loader V3 ProgramData but its \
                     prefix is not a ProgramData variant",
                        base58(&position.key)
                    )));
                };
                match self.deployment_slots.get(&position.key).copied() {
                    Some(known) if known != slot => {
                        return Err(self.refuse(format!(
                            "position {set_index} ({}) was redeployed: deployment_slot moved from \
                         {known} to {slot}. The pinned elf_digest no longer describes the \
                         program, so this set stops being attested (\u{a7}4.11); the market's \
                         funded failure path is the correct handling",
                            base58(&position.key)
                        )));
                    }
                    _ => {}
                }
                self.deployment_slots.insert(position.key, slot);
                Some(slot)
            } else {
                None
            };

        if u64::from(position.inline_len) == account.data_len {
            // Fully inline: the tail is empty and its digest is the pinned
            // empty-string digest.  This is not a variant, it is the case
            // `inline_len == data_len`.
            return Ok(PreparedPosition {
                data_len,
                inline,
                tail_digest: SHA256_EMPTY_DIGEST,
                source: TailDigestSource::FullyInline,
                pages: 0,
            });
        }

        if let Some(deployment_slot) = programdata_slot
            && let Some(cached) = self
                .tail_cache
                .get(&(position.key, deployment_slot))
                .copied()
        {
            return Ok(PreparedPosition {
                data_len,
                inline,
                tail_digest: cached,
                source: TailDigestSource::Cached { deployment_slot },
                pages: 0,
            });
        }

        let (tail_digest, pages, bytes) = self
            .page_tail_digest(rpc, position, &inline, account.data_len, observed_slot)
            .await?;
        if let Some(deployment_slot) = programdata_slot {
            self.tail_cache
                .insert((position.key, deployment_slot), tail_digest);
        }
        Ok(PreparedPosition {
            data_len,
            inline,
            tail_digest,
            source: TailDigestSource::Paged { pages, bytes },
            pages,
        })
    }

    /// Page one account body and hash its tail.
    ///
    /// These `getAccountInfo` calls are not the observation and carry no slot
    /// into any signed message: the observation's slot came from the one batch
    /// call.  What makes the paging safe is that the first page must reproduce
    /// the pinned inline prefix **byte for byte** as the batch read it — a body
    /// that moved between calls fails that check instead of being folded in.
    async fn page_tail_digest(
        &mut self,
        rpc: &RpcClient,
        position: &PositionConfig,
        expected_inline: &[u8],
        data_len: u64,
        observed_slot: u64,
    ) -> Result<([u8; ID_BYTES], u32, u64)> {
        let page_bytes = u64::try_from(self.body_page_bytes).unwrap_or(u64::MAX);
        let inline_len = u64::from(position.inline_len);
        let mut hasher = TailHasher::new();
        let mut offset = 0u64;
        let mut pages = 0u32;

        while offset < data_len {
            if pages >= MAX_BODY_PAGES {
                return Err(self.refuse(format!(
                    "paging {} exceeded the {MAX_BODY_PAGES}-page bound",
                    base58(&position.key)
                )));
            }
            let length = page_bytes.min(data_len.saturating_sub(offset));
            let page = rpc
                .get_account_page(&position.key, offset, length, Some(observed_slot))
                .await?;
            pages = pages.saturating_add(1);
            let Some(account) = page.account else {
                return Err(self.refuse(format!(
                    "{} vanished while its body was being paged",
                    base58(&position.key)
                )));
            };
            if account.data_len != data_len {
                return Err(self.refuse(format!(
                    "{} changed width from {data_len} to {} while its body was being paged",
                    base58(&position.key),
                    account.data_len
                )));
            }
            if u64::try_from(account.data.len()).unwrap_or(u64::MAX) != length {
                return Err(self.refuse(format!(
                    "a page of {} returned {} bytes where {length} were asked for",
                    base58(&position.key),
                    account.data.len()
                )));
            }
            let tail = if offset == 0 {
                let Some(prefix) = account.data.get(..expected_inline.len()) else {
                    return Err(self.refuse(format!(
                        "the first page of {} was shorter than its pinned inline prefix",
                        base58(&position.key)
                    )));
                };
                if prefix != expected_inline {
                    return Err(self.refuse(format!(
                        "{} changed between the batch read and its body paging: the pinned inline \
                         prefix does not match",
                        base58(&position.key)
                    )));
                }
                account.data.get(expected_inline.len()..).unwrap_or(&[])
            } else {
                account.data.as_slice()
            };
            hasher.absorb(tail);
            offset = offset.saturating_add(length);
        }

        let expected_tail = data_len.saturating_sub(inline_len);
        if hasher.absorbed() != expected_tail {
            return Err(self.refuse(format!(
                "paging {} hashed {} tail bytes where {expected_tail} were expected",
                base58(&position.key),
                hasher.absorbed()
            )));
        }
        let absorbed = hasher.absorbed();
        Ok((hasher.finish(), pages, absorbed))
    }
}

#[derive(Debug)]
struct PreparedPosition {
    data_len: u32,
    inline: Vec<u8>,
    tail_digest: [u8; ID_BYTES],
    source: TailDigestSource,
    pages: u32,
}

/// Compare one position across two endpoints.
///
/// When the two reads landed on the same slot the comparison is byte-exact.
/// When they did not, only the facts that a slot difference cannot legitimately
/// change are compared — existence, owner, executable and width — because a
/// mutable pool's bytes differing across slots is not a disagreement, and
/// treating it as one would stop every healthy set.
pub fn disagreement(
    mine: Option<&ObservedAccount>,
    theirs: Option<&ObservedAccount>,
    same_slot: bool,
) -> Option<String> {
    match (mine, theirs) {
        (None, None) => None,
        (Some(_), None) => Some("the account exists here and not there".to_owned()),
        (None, Some(_)) => Some("the account exists there and not here".to_owned()),
        (Some(mine), Some(theirs)) => {
            if mine.owner != theirs.owner {
                return Some(format!(
                    "owner {} against {}",
                    base58(&mine.owner),
                    base58(&theirs.owner)
                ));
            }
            if mine.executable != theirs.executable {
                return Some("executable flag differs".to_owned());
            }
            if mine.data_len != theirs.data_len {
                return Some(format!(
                    "data_len {} against {}",
                    mine.data_len, theirs.data_len
                ));
            }
            if same_slot {
                if mine.lamports != theirs.lamports {
                    return Some(format!(
                        "lamports {} against {} at the same slot",
                        mine.lamports, theirs.lamports
                    ));
                }
                if mine.data != theirs.data {
                    return Some("account bytes differ at the same slot".to_owned());
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::LOADER_V3_PROGRAM_ID;
    use crate::config::AccountSetConfig;

    fn programdata_prefix(deployment_slot: u64) -> Vec<u8> {
        let mut prefix = vec![0u8; 45];
        prefix[..4].copy_from_slice(&3u32.to_le_bytes());
        prefix[4..12].copy_from_slice(&deployment_slot.to_le_bytes());
        prefix[12] = 1;
        prefix
    }

    fn programdata_account(deployment_slot: u64) -> ObservedAccount {
        ObservedAccount {
            lamports: 1_000_000,
            owner: LOADER_V3_PROGRAM_ID,
            executable: false,
            data: programdata_prefix(deployment_slot),
            data_len: 2_300_000,
        }
    }

    fn watcher() -> SetWatcher {
        let position = PositionConfig {
            key: [0x11; ID_BYTES],
            expected_owner: LOADER_V3_PROGRAM_ID,
            inline_len: 45,
            admitted_data_lens: Vec::new(),
        };
        let config = AccountSetConfig {
            name: "dbc".to_owned(),
            relay_family_id: dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1,
            decoding_rules_id: dclutch_relay_contract::RELAYED_DECODING_RULES_SCHEMA_RELEASE_ID_V1,
            positions: vec![position],
            account_set_id: [0x5a; ID_BYTES],
        };
        SetWatcher::new(
            config,
            dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1,
            256 * 1024,
        )
    }

    /// The §4.11 stop rule, executed: a redeploy changes `deployment_slot` in
    /// the 45-byte prefix the batch already fetched, and the set stops.  It
    /// stays stopped; nothing in the daemon un-stops it.
    #[tokio::test]
    async fn a_deployment_slot_change_stops_the_set_and_it_stays_stopped() {
        let rpc = RpcClient::new(
            "http://127.0.0.1:1",
            std::time::Duration::from_millis(1),
            None,
        )
        .expect("client");
        let mut watcher = watcher();
        let position = watcher
            .config()
            .positions
            .first()
            .expect("position")
            .clone();

        // First cycle records the deployment slot.  The account is not fully
        // inline, so preparation would page; the assertion is only that the
        // *slot bookkeeping* accepted it, which it does before any paging.
        watcher.seed_deployment_slot(position.key, 360_000_000);

        let error = watcher
            .prepare_position(&rpc, 0, &position, &programdata_account(360_000_777), 45, 1)
            .await
            .expect_err("a redeploy must refuse");
        match &error {
            RelayerError::ObservationRefused { set, reason } => {
                assert_eq!(set, "dbc");
                assert!(reason.contains("redeployed"), "{reason}");
                assert!(
                    reason.contains("360000000")
                        || reason.contains("360_000_000")
                        || reason.contains("360000777"),
                    "{reason}"
                );
            }
            other => panic!("expected an observation refusal, got {other:?}"),
        }
        assert!(watcher.stopped_reason().is_some());

        // And it stays stopped: a later cycle refuses without touching the RPC.
        let signer_dir = tempfile::tempdir().expect("tempdir");
        let path = signer_dir.path().join("k.json");
        crate::keys::generate_keypair_file(&path, None).expect("keygen");
        let signer = AttestationSigner::load(&path, None).expect("load");
        let again = watcher
            .observe(&rpc, &[], &signer)
            .await
            .expect_err("stopped");
        assert!(matches!(again, RelayerError::ObservationRefused { .. }));
    }

    #[tokio::test]
    async fn a_data_len_outside_the_admitted_set_stops_the_set() {
        let rpc = RpcClient::new(
            "http://127.0.0.1:1",
            std::time::Duration::from_millis(1),
            None,
        )
        .expect("client");
        let mut watcher = watcher();
        let mut position = watcher
            .config()
            .positions
            .first()
            .expect("position")
            .clone();
        position.admitted_data_lens = vec![2_300_000];
        let mut account = programdata_account(1);
        account.data_len = 2_300_001;
        let error = watcher
            .prepare_position(&rpc, 0, &position, &account, 45, 1)
            .await
            .expect_err("an inadmissible width must refuse");
        assert!(
            matches!(error, RelayerError::ObservationRefused { .. }),
            "{error:?}"
        );
        assert!(watcher.stopped_reason().is_some());
    }

    #[tokio::test]
    async fn a_substituted_owner_stops_the_set() {
        let rpc = RpcClient::new(
            "http://127.0.0.1:1",
            std::time::Duration::from_millis(1),
            None,
        )
        .expect("client");
        let mut watcher = watcher();
        let position = watcher
            .config()
            .positions
            .first()
            .expect("position")
            .clone();
        let mut account = programdata_account(1);
        account.owner = [0x99; ID_BYTES];
        assert!(
            watcher
                .prepare_position(&rpc, 0, &position, &account, 45, 1)
                .await
                .is_err()
        );
        assert!(watcher.stopped_reason().is_some());
    }

    #[tokio::test]
    async fn a_fully_inline_position_needs_no_paging_and_takes_the_empty_digest() {
        let rpc = RpcClient::new(
            "http://127.0.0.1:1",
            std::time::Duration::from_millis(1),
            None,
        )
        .expect("client");
        let mut watcher = watcher();
        let position = PositionConfig {
            key: [0x22; ID_BYTES],
            expected_owner: [0x33; ID_BYTES],
            inline_len: 4,
            admitted_data_lens: vec![4],
        };
        let account = ObservedAccount {
            lamports: 5,
            owner: [0x33; ID_BYTES],
            executable: false,
            data: vec![1, 2, 3, 4],
            data_len: 4,
        };
        let prepared = watcher
            .prepare_position(&rpc, 0, &position, &account, 4, 1)
            .await
            .expect("fully inline");
        assert_eq!(prepared.tail_digest, SHA256_EMPTY_DIGEST);
        assert_eq!(prepared.source, TailDigestSource::FullyInline);
        assert_eq!(prepared.pages, 0);
        assert!(watcher.stopped_reason().is_none());
    }

    #[tokio::test]
    async fn a_cached_tail_digest_is_reused_for_the_same_deployment_slot() {
        let rpc = RpcClient::new(
            "http://127.0.0.1:1",
            std::time::Duration::from_millis(1),
            None,
        )
        .expect("client");
        let mut watcher = watcher();
        let position = watcher
            .config()
            .positions
            .first()
            .expect("position")
            .clone();
        watcher
            .tail_cache
            .insert((position.key, 360_000_000), [0xEE; ID_BYTES]);
        let prepared = watcher
            .prepare_position(&rpc, 0, &position, &programdata_account(360_000_000), 45, 1)
            .await
            .expect("cache hit");
        assert_eq!(prepared.tail_digest, [0xEE; ID_BYTES]);
        assert_eq!(
            prepared.source,
            TailDigestSource::Cached {
                deployment_slot: 360_000_000
            }
        );
        assert_eq!(prepared.pages, 0, "a cache hit must not touch the network");
    }

    #[test]
    fn cross_endpoint_disagreement_is_byte_exact_only_at_a_shared_slot() {
        let base = ObservedAccount {
            lamports: 10,
            owner: [1; ID_BYTES],
            executable: false,
            data: vec![1, 2, 3],
            data_len: 3,
        };
        let mut different_bytes = base.clone();
        different_bytes.data = vec![9, 9, 9];

        assert!(disagreement(Some(&base), Some(&different_bytes), true).is_some());
        assert!(disagreement(Some(&base), Some(&different_bytes), false).is_none());

        let mut different_owner = base.clone();
        different_owner.owner = [2; ID_BYTES];
        assert!(disagreement(Some(&base), Some(&different_owner), false).is_some());

        let mut different_width = base.clone();
        different_width.data_len = 4;
        assert!(disagreement(Some(&base), Some(&different_width), false).is_some());

        assert!(disagreement(Some(&base), None, true).is_some());
        assert!(disagreement(None, None, true).is_none());
        assert!(disagreement(Some(&base), Some(&base), true).is_none());
    }
}
