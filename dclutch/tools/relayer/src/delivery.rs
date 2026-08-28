//! Finalized-state reconciliation and durable evidence for relay delivery.
//!
//! A transaction signature is not an acknowledgement.  The RPC response can
//! be lost after the transaction lands, signatures change when a blockhash is
//! replaced, and a daemon can crash between either event and its next write.
//! The acknowledgement is therefore the finalized observation-record bytes.
//! This module checks those bytes against the complete expected binding and
//! every already accepted body before it says what action, if any, remains.

use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use dclutch_relay_contract::record::{RelayedObservationRecordViewV1, RelayedRecordPhaseV1};

use crate::chain::{LOADER_V3_PROGRAM_ID, program_programdata_link, programdata_deployment_slot};
use crate::derive::{SetDigestFold, sha256};
use crate::error::{RelayerError, Result};
use crate::id32::{ID_BYTES, base58, to_hex};
use crate::rpc::ObservedAccount;

/// Exact live accounts that a send-capable relay launch pins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchExpectation {
    /// Relay Program address.
    pub relay_program_id: [u8; ID_BYTES],
    /// Linked ProgramData address.
    pub relay_program_data: [u8; ID_BYTES],
    /// Accepted deployment slot under decision 0012.
    pub relay_program_deployment_slot: u64,
    /// Live Market address.
    pub market: [u8; ID_BYTES],
    /// Program expected to own the live Market.
    pub market_owner: [u8; ID_BYTES],
}

/// Check the finalized Program -> ProgramData slot pin and live Market shape.
pub fn require_live_launch_accounts(
    expected: LaunchExpectation,
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    market: &ObservedAccount,
) -> Result<()> {
    if program.owner != LOADER_V3_PROGRAM_ID
        || !program.executable
        || program.data_len != 36
        || program_programdata_link(&program.data) != Some(expected.relay_program_data)
    {
        return Err(RelayerError::MissingCapability(
            "relay Program -> ProgramData link or Loader-v3 executable shape differs from the \
             launch capability"
                .to_owned(),
        ));
    }
    if programdata.owner != LOADER_V3_PROGRAM_ID
        || programdata.executable
        || programdata_deployment_slot(&programdata.data)
            != Some(expected.relay_program_deployment_slot)
    {
        return Err(RelayerError::MissingCapability(format!(
            "relay ProgramData does not carry accepted deployment slot {} under Loader-v3",
            expected.relay_program_deployment_slot
        )));
    }
    if market.owner != expected.market_owner || market.executable || market.data_len == 0 {
        return Err(RelayerError::MissingCapability(
            "Market is absent or does not have the capability-pinned live owner shape".to_owned(),
        ));
    }
    Ok(())
}

/// Complete immutable identity and content expected of one delivered set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryExpectation {
    /// Submit-cluster genesis hash.
    pub submit_cluster_id: [u8; ID_BYTES],
    /// Relay program owning the record account.
    pub relay_program_id: [u8; ID_BYTES],
    /// Owning Market.
    pub market: [u8; ID_BYTES],
    /// Market generation.
    pub generation: u64,
    /// Immutable Source material.
    pub source_material_id: [u8; ID_BYTES],
    /// Ordered observed account set.
    pub account_set_id: [u8; ID_BYTES],
    /// Provider release authorizing the signer set and decoding rules.
    pub provider_release_id: [u8; ID_BYTES],
    /// Content-addressed immutable relayer key set.
    pub relayer_key_set_id: [u8; ID_BYTES],
    /// Genesis hash claimed by the foreign-cluster observation.
    pub observed_cluster_id: [u8; ID_BYTES],
    /// Finalized foreign slot carried by every attestation.
    pub observed_slot: u64,
    /// Exact observation bodies, in set order.
    pub bodies: Vec<Vec<u8>>,
    /// Final set digest carried by the signed seal.
    pub set_digest: [u8; ID_BYTES],
}

/// The next action derived from finalized state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryAction {
    /// The record account does not exist yet; its creator has not run.
    AwaitRecord,
    /// Append this exact next set position.
    Append(u16),
    /// Every body is present; submit the seal.
    Seal,
    /// The exact set is sealed or has subsequently been consumed.
    Complete,
}

/// Reconcile one finalized record account against the expected delivery.
///
/// `record_owner` and `record_data` are both `None` for a vacant address and
/// both `Some` for an existing account.  Any mixed shape is refused.  Existing
/// bytes must be owned by the pinned relay program, carry every immutable
/// binding exactly, and contain a byte-identical prefix of the signed bodies.
pub fn reconcile_finalized_record(
    expected: &DeliveryExpectation,
    record_owner: Option<[u8; ID_BYTES]>,
    record_data: Option<&[u8]>,
) -> Result<DeliveryAction> {
    let (owner, data) = match (record_owner, record_data) {
        (None, None) => return Ok(DeliveryAction::AwaitRecord),
        (Some(owner), Some(data)) => (owner, data),
        _ => {
            return Err(RelayerError::config(
                "record acknowledgement carried an impossible owner/data presence shape",
            ));
        }
    };
    if owner != expected.relay_program_id {
        return Err(RelayerError::config(format!(
            "forged record acknowledgement: owner {} is not pinned relay program {}",
            base58(&owner),
            base58(&expected.relay_program_id)
        )));
    }
    let view = RelayedObservationRecordViewV1::decode(data)
        .map_err(|error| RelayerError::wire("finalized record acknowledgement", error))?;
    require_binding(expected, view)?;

    let expected_count = u16::try_from(expected.bodies.len())
        .map_err(|_| RelayerError::config("delivery body count does not fit in u16"))?;
    if view.set_count().map_err(record_wire)? != expected_count {
        return Err(RelayerError::config(format!(
            "record acknowledgement set_count does not match signed body count {expected_count}"
        )));
    }
    let filled = view.filled_count().map_err(record_wire)?;
    let mut fold = SetDigestFold::seed(expected.account_set_id, expected.observed_slot)?;
    for index in 0..filled {
        let expected_body = expected
            .bodies
            .get(usize::from(index))
            .ok_or_else(|| RelayerError::config("record filled_count exceeds signed body count"))?;
        let observed = view.observation(index).map_err(record_wire)?;
        let mut encoded = vec![0u8; observed.encoded_len()];
        observed
            .encode_into(&mut encoded)
            .map_err(|error| RelayerError::wire("encode finalized observation", error))?;
        if encoded != *expected_body {
            return Err(RelayerError::config(format!(
                "forged or conflicting record acknowledgement: accepted body at position {index} \
                 differs from the signed delivery"
            )));
        }
        fold.absorb(expected_body);
    }
    if view.set_digest().map_err(record_wire)? != fold.digest() {
        return Err(RelayerError::config(
            "forged or corrupt record acknowledgement: running set digest does not match the \
             byte-exact accepted prefix",
        ));
    }

    match view.phase().map_err(record_wire)? {
        RelayedRecordPhaseV1::Collecting if filled < expected_count => {
            Ok(DeliveryAction::Append(filled))
        }
        RelayedRecordPhaseV1::Collecting => {
            if fold.digest() != expected.set_digest {
                return Err(RelayerError::config(
                    "complete record prefix does not match the signed seal digest",
                ));
            }
            Ok(DeliveryAction::Seal)
        }
        RelayedRecordPhaseV1::Sealed | RelayedRecordPhaseV1::Consumed => {
            if filled != expected_count || fold.digest() != expected.set_digest {
                return Err(RelayerError::config(
                    "terminal record acknowledgement does not match the complete signed set",
                ));
            }
            Ok(DeliveryAction::Complete)
        }
        RelayedRecordPhaseV1::Retired => Err(RelayerError::config(
            "record retired before this delivery obtained a byte-exact terminal acknowledgement",
        )),
    }
}

fn record_wire(error: dclutch_relay_contract::Error) -> RelayerError {
    RelayerError::wire("finalized record acknowledgement", error)
}

fn require_binding(
    expected: &DeliveryExpectation,
    view: RelayedObservationRecordViewV1<'_>,
) -> Result<()> {
    let exact = view.market().map_err(record_wire)? == expected.market
        && view.generation().map_err(record_wire)? == expected.generation
        && view.source_material_id().map_err(record_wire)? == expected.source_material_id
        && view.account_set_id().map_err(record_wire)? == expected.account_set_id
        && view.provider_release_id().map_err(record_wire)? == expected.provider_release_id
        && view.relayer_key_set_id().map_err(record_wire)? == expected.relayer_key_set_id
        && view.observed_cluster_id().map_err(record_wire)? == expected.observed_cluster_id
        && view.observed_slot().map_err(record_wire)? == expected.observed_slot;
    if !exact {
        return Err(RelayerError::config(
            "forged or stale record acknowledgement: immutable market/generation/source/set/\
             release/key-set/cluster/slot binding differs from the accepted delivery",
        ));
    }
    Ok(())
}

/// One durable delivery evidence stream.
///
/// Entries are immutable numbered files, written through a same-directory
/// temporary file and renamed into place.  A crash before rename leaves no
/// accepted entry; a crash after rename leaves the complete entry.  The chain
/// remains the authority and this journal records how the daemon converged on
/// it.
#[derive(Debug)]
pub struct DeliveryJournal {
    dir: PathBuf,
    next_sequence: u64,
    previous_sha256: [u8; ID_BYTES],
}

impl DeliveryJournal {
    /// Open or resume the journal for one record address.
    pub fn open(output_dir: &Path, record: [u8; ID_BYTES]) -> Result<Self> {
        let dir = output_dir.join("delivery-journal").join(base58(&record));
        std::fs::create_dir_all(&dir).map_err(|source| RelayerError::io(&dir, source))?;
        let mut sequence = 0u64;
        let mut previous = [0u8; ID_BYTES];
        loop {
            let path = entry_path(&dir, sequence);
            if !path.exists() {
                break;
            }
            let bytes = std::fs::read(&path).map_err(|source| RelayerError::io(&path, source))?;
            let value: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|error| RelayerError::Serialization(error.to_string()))?;
            let recorded_sequence = value
                .get("sequence")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| RelayerError::config("delivery journal entry has no sequence"))?;
            let recorded_previous = value
                .get("previous_sha256_hex")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    RelayerError::config("delivery journal entry has no previous_sha256_hex")
                })?;
            if recorded_sequence != sequence || recorded_previous != to_hex(&previous) {
                return Err(RelayerError::config(format!(
                    "delivery journal chain breaks at {}",
                    path.display()
                )));
            }
            previous = sha256(&bytes);
            sequence = sequence
                .checked_add(1)
                .ok_or_else(|| RelayerError::config("delivery journal sequence overflowed"))?;
        }
        Ok(Self {
            dir,
            next_sequence: sequence,
            previous_sha256: previous,
        })
    }

    /// Append one event after its authoritative action or observation.
    pub fn record(&mut self, event: &str, detail: serde_json::Value) -> Result<()> {
        let value = serde_json::json!({
            "schema": "dclutch.relayer.delivery-journal.v1",
            "sequence": self.next_sequence,
            "previous_sha256_hex": to_hex(&self.previous_sha256),
            "event": event,
            "detail": detail,
        });
        let bytes = serde_json::to_vec_pretty(&value)
            .map_err(|error| RelayerError::Serialization(error.to_string()))?;
        let final_path = entry_path(&self.dir, self.next_sequence);
        let temp_path = self.dir.join(format!(".{:020}.tmp", self.next_sequence));
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temp_path)
            .map_err(|source| RelayerError::io(&temp_path, source))?;
        file.write_all(&bytes)
            .map_err(|source| RelayerError::io(&temp_path, source))?;
        file.sync_all()
            .map_err(|source| RelayerError::io(&temp_path, source))?;
        std::fs::rename(&temp_path, &final_path)
            .map_err(|source| RelayerError::io(&final_path, source))?;
        self.previous_sha256 = sha256(&bytes);
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| RelayerError::config("delivery journal sequence overflowed"))?;
        Ok(())
    }
}

fn entry_path(dir: &Path, sequence: u64) -> PathBuf {
    dir.join(format!("{sequence:020}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_relay_contract::record::{
        RelayedRecordBindingV1, append_relayed_observation_in_place_v1,
        create_relayed_observation_record_into_v1, relayed_observation_record_bytes_v1,
        seal_relayed_observation_in_place_v1,
    };
    use dclutch_relay_contract::wire::{
        AccountObservationV1, AttestationMessageV1, ObservationSetSealV1,
    };

    const PROGRAM: [u8; ID_BYTES] = [0x11; ID_BYTES];
    const MARKET: [u8; ID_BYTES] = [0x12; ID_BYTES];
    const SOURCE: [u8; ID_BYTES] = [0x13; ID_BYTES];
    const SET: [u8; ID_BYTES] = [0x14; ID_BYTES];
    const RELEASE: [u8; ID_BYTES] = [0x15; ID_BYTES];
    const KEYS: [u8; ID_BYTES] = [0x16; ID_BYTES];
    const OBSERVED_CLUSTER: [u8; ID_BYTES] = [0x17; ID_BYTES];
    const SLOT: u64 = 423_941_138;

    fn body(key: u8) -> Vec<u8> {
        let inline = [key, key.wrapping_add(1)];
        let observation = AccountObservationV1::new(
            [key; ID_BYTES],
            [0x22; ID_BYTES],
            1_000_000,
            2,
            &inline,
            false,
            dclutch_relay_contract::SHA256_EMPTY_DIGEST,
        )
        .expect("body");
        let mut encoded = vec![0u8; observation.encoded_len()];
        observation.encode_into(&mut encoded).expect("encode body");
        encoded
    }

    fn binding() -> RelayedRecordBindingV1 {
        RelayedRecordBindingV1 {
            market: MARKET,
            generation: 4,
            source_material_id: SOURCE,
            account_set_id: SET,
            provider_release_id: RELEASE,
            relayer_key_set_id: KEYS,
            observed_cluster_id: OBSERVED_CLUSTER,
            observed_slot: SLOT,
        }
    }

    fn expectation() -> DeliveryExpectation {
        let bodies = vec![body(0x31), body(0x41)];
        let mut fold = SetDigestFold::seed(SET, SLOT).expect("seed");
        for body in &bodies {
            fold.absorb(body);
        }
        DeliveryExpectation {
            submit_cluster_id: [0x18; ID_BYTES],
            relay_program_id: PROGRAM,
            market: MARKET,
            generation: 4,
            source_material_id: SOURCE,
            account_set_id: SET,
            provider_release_id: RELEASE,
            relayer_key_set_id: KEYS,
            observed_cluster_id: OBSERVED_CLUSTER,
            observed_slot: SLOT,
            bodies,
            set_digest: fold.digest(),
        }
    }

    fn collecting_record(filled: u16) -> Vec<u8> {
        let expected = expectation();
        let mut running = SetDigestFold::seed(SET, SLOT).expect("seed");
        let mut record = vec![0u8; relayed_observation_record_bytes_v1(2).expect("record width")];
        create_relayed_observation_record_into_v1(
            &mut record,
            binding(),
            [0x19; ID_BYTES],
            2,
            1,
            running.digest(),
            1_700_000_000,
        )
        .expect("create");
        for index in 0..filled {
            let encoded = expected.bodies.get(usize::from(index)).expect("body");
            let observation = AccountObservationV1::decode(encoded).expect("decode body");
            let message = AttestationMessageV1::new(
                OBSERVED_CLUSTER,
                [0x20; ID_BYTES],
                [0x21; ID_BYTES],
                SET,
                SLOT,
                index,
                2,
                observation,
            )
            .expect("message");
            running.absorb(encoded);
            append_relayed_observation_in_place_v1(
                &mut record,
                binding(),
                message,
                running.digest(),
            )
            .expect("append");
        }
        record
    }

    #[test]
    fn restart_and_duplicate_delivery_resume_at_the_exact_next_position() {
        let expected = expectation();
        let record = collecting_record(1);
        let first =
            reconcile_finalized_record(&expected, Some(PROGRAM), Some(&record)).expect("reconcile");
        let after_restart = reconcile_finalized_record(&expected, Some(PROGRAM), Some(&record))
            .expect("reconcile after restart");
        assert_eq!(first, DeliveryAction::Append(1));
        assert_eq!(after_restart, first);
    }

    #[test]
    fn a_lost_send_response_is_reconciled_from_the_landed_finalized_state() {
        let expected = expectation();
        // The caller intentionally supplies no transaction signature or send
        // result here. The only input is the state that exists after a remote
        // node accepted the bytes and lost its HTTP response.
        let landed = collecting_record(1);
        assert_eq!(
            reconcile_finalized_record(&expected, Some(PROGRAM), Some(&landed))
                .expect("finalized state is the acknowledgement"),
            DeliveryAction::Append(1)
        );
    }

    #[test]
    fn reordering_or_a_forged_ack_never_skips_an_append() {
        let mut expected = expectation();
        let record = collecting_record(1);
        expected.bodies.swap(0, 1);
        let error = reconcile_finalized_record(&expected, Some(PROGRAM), Some(&record))
            .expect_err("reordered expectation must refuse");
        assert!(error.to_string().contains("accepted body at position 0"));

        let error =
            reconcile_finalized_record(&expectation(), Some([0x99; ID_BYTES]), Some(&record))
                .expect_err("wrong owner must refuse");
        assert!(error.to_string().contains("forged record acknowledgement"));
    }

    #[test]
    fn wrong_provider_release_refuses_even_when_every_body_matches() {
        let record = collecting_record(1);
        let mut expected = expectation();
        expected.provider_release_id = [0x77; ID_BYTES];
        let error = reconcile_finalized_record(&expected, Some(PROGRAM), Some(&record))
            .expect_err("wrong release must refuse");
        assert!(error.to_string().contains("immutable market/generation"));
    }

    #[test]
    fn a_complete_exact_record_advances_to_seal_then_complete() {
        let expected = expectation();
        let mut record = collecting_record(2);
        assert_eq!(
            reconcile_finalized_record(&expected, Some(PROGRAM), Some(&record))
                .expect("complete collecting"),
            DeliveryAction::Seal
        );
        let seal = ObservationSetSealV1::new(
            OBSERVED_CLUSTER,
            [0x20; ID_BYTES],
            SET,
            SLOT,
            2,
            expected.set_digest,
        )
        .expect("seal");
        seal_relayed_observation_in_place_v1(&mut record, binding(), seal, 0, 1_700_000_001)
            .expect("seal record");
        assert_eq!(
            reconcile_finalized_record(&expected, Some(PROGRAM), Some(&record)).expect("sealed"),
            DeliveryAction::Complete
        );
    }

    #[test]
    fn wrong_program_slot_refuses_the_launch_contract() {
        let program_data_key = [0x52; ID_BYTES];
        let mut program_bytes = vec![0u8; 36];
        program_bytes[..4].copy_from_slice(&2u32.to_le_bytes());
        program_bytes[4..].copy_from_slice(&program_data_key);
        let program = ObservedAccount {
            lamports: 1,
            owner: LOADER_V3_PROGRAM_ID,
            executable: true,
            data: program_bytes,
            data_len: 36,
        };
        let mut programdata_bytes = vec![0u8; 45];
        programdata_bytes[..4].copy_from_slice(&3u32.to_le_bytes());
        programdata_bytes[4..12].copy_from_slice(&99u64.to_le_bytes());
        let programdata = ObservedAccount {
            lamports: 1,
            owner: LOADER_V3_PROGRAM_ID,
            executable: false,
            data: programdata_bytes,
            data_len: 45,
        };
        let market = ObservedAccount {
            lamports: 1,
            owner: [0x53; ID_BYTES],
            executable: false,
            data: vec![1],
            data_len: 1,
        };
        let error = require_live_launch_accounts(
            LaunchExpectation {
                relay_program_id: PROGRAM,
                relay_program_data: program_data_key,
                relay_program_deployment_slot: 100,
                market: MARKET,
                market_owner: market.owner,
            },
            &program,
            &programdata,
            &market,
        )
        .expect_err("wrong slot must refuse");
        assert!(error.to_string().contains("accepted deployment slot 100"));
    }

    #[test]
    fn journal_restart_continues_the_hash_chain_without_rewriting() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record = [0x88; ID_BYTES];
        let mut first = DeliveryJournal::open(temp.path(), record).expect("open");
        first
            .record("one", serde_json::json!({ "value": 1 }))
            .expect("record one");
        drop(first);
        let mut restarted = DeliveryJournal::open(temp.path(), record).expect("restart");
        restarted
            .record("two", serde_json::json!({ "value": 2 }))
            .expect("record two");
        let dir = temp.path().join("delivery-journal").join(base58(&record));
        assert!(entry_path(&dir, 0).exists());
        assert!(entry_path(&dir, 1).exists());
    }
}
