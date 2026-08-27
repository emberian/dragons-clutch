// Integration-test crate: clippy's `allow-*-in-tests` settings only reach
// `#[cfg(test)]` modules, so the same test-only ergonomics are allowed here
// explicitly.  Non-test code in `src/` is held to the full bar.
#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::panic
)]

//! One complete observation cycle against a local mock RPC.
//!
//! Everything here is loopback: the test binds `127.0.0.1:0` and serves fixed
//! JSON-RPC responses.  No public cluster is contacted, nothing is submitted,
//! and the signing key is generated inside the test.
//!
//! What it proves that the unit tests do not: the batch read, the pinned-prefix
//! truncation, the paged tail digest, the fold, the attestation encode/sign
//! round trip through the wire crate's own decoder, the seal, the artifact
//! directory and the publication log all agree end to end.

use std::path::Path;
use std::time::Duration;

use dclutch_relay_contract::wire::{AttestationMessageV1, ObservationSetSealV1};
use dclutch_relay_contract::{RELAYED_SEAL_BYTES, SHA256_EMPTY_DIGEST};
use dclutch_relayer::artifacts::ArtifactWriter;
use dclutch_relayer::chain::LOADER_V3_PROGRAM_ID;
use dclutch_relayer::config::{AccountSetConfig, PositionConfig};
use dclutch_relayer::derive::{SetDigestFold, derive_account_set_id, sha256};
use dclutch_relayer::id32::base58;
use dclutch_relayer::keys::{AttestationSigner, generate_keypair_file};
use dclutch_relayer::observe::{SetWatcher, TailDigestSource};
use dclutch_relayer::publog::{MessageKind, PublicationLog};
use dclutch_relayer::rpc::RpcClient;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const CLUSTER: [u8; 32] = dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1;
const FAMILY: [u8; 32] = dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1;
const RULES: [u8; 32] = dclutch_relay_contract::RELAYED_DECODING_RULES_SCHEMA_RELEASE_ID_V1;

const POOL_KEY: [u8; 32] = [0x11; 32];
const POOL_OWNER: [u8; 32] = [0x22; 32];
const PROGRAMDATA_KEY: [u8; 32] = [0x33; 32];
const DEPLOYMENT_SLOT: u64 = 360_000_000;
const PROGRAMDATA_LEN: usize = 1045;
const OBSERVED_SLOT: u64 = 423_941_138;

fn pool_bytes() -> Vec<u8> {
    vec![0xA1, 0xA2, 0xA3, 0xA4]
}

fn programdata_bytes() -> Vec<u8> {
    let mut bytes = vec![0u8; PROGRAMDATA_LEN];
    bytes[..4].copy_from_slice(&3u32.to_le_bytes());
    bytes[4..12].copy_from_slice(&DEPLOYMENT_SLOT.to_le_bytes());
    bytes[12] = 1;
    bytes[13..45].copy_from_slice(&[0x7Eu8; 32]);
    for (index, byte) in bytes.iter_mut().enumerate().skip(45) {
        *byte = u8::try_from(index % 251).unwrap_or(0);
    }
    bytes
}

fn encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn account_json(bytes: &[u8], full_len: usize, owner: &[u8; 32], executable: bool) -> Value {
    json!({
        "lamports": 1_000_000u64 + full_len as u64,
        "owner": base58(owner),
        "executable": executable,
        "rentEpoch": 0,
        "space": full_len,
        "data": [encode(bytes), "base64"],
    })
}

fn slice_of(full: &[u8], offset: usize, length: usize) -> Vec<u8> {
    let end = offset.saturating_add(length).min(full.len());
    full.get(offset.min(full.len())..end)
        .unwrap_or(&[])
        .to_vec()
}

fn respond(request: &Value) -> Value {
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request
        .get("params")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    match method {
        "getGenesisHash" => json!({ "jsonrpc": "2.0", "id": 1, "result": base58(&CLUSTER) }),
        "getMultipleAccounts" => {
            let length = params
                .get(1)
                .and_then(|config| config.get("dataSlice"))
                .and_then(|slice| slice.get("length"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let pool = pool_bytes();
            let programdata = programdata_bytes();
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "context": { "apiVersion": "3.0.0", "slot": OBSERVED_SLOT },
                    "value": [
                        account_json(&slice_of(&pool, 0, length), pool.len(), &POOL_OWNER, false),
                        account_json(
                            &slice_of(&programdata, 0, length),
                            programdata.len(),
                            &LOADER_V3_PROGRAM_ID,
                            false,
                        ),
                    ],
                },
            })
        }
        "getAccountInfo" => {
            let slice = params.get(1).and_then(|config| config.get("dataSlice"));
            let offset = slice
                .and_then(|s| s.get("offset"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let length = slice
                .and_then(|s| s.get("length"))
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize;
            let programdata = programdata_bytes();
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "context": { "apiVersion": "3.0.0", "slot": OBSERVED_SLOT },
                    "value": account_json(
                        &slice_of(&programdata, offset, length),
                        programdata.len(),
                        &LOADER_V3_PROGRAM_ID,
                        false,
                    ),
                },
            })
        }
        other => json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": { "code": -32601, "message": format!("unmocked method {other}") },
        }),
    }
}

/// A single-threaded loopback JSON-RPC server good enough for reqwest.
async fn spawn_mock() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                let mut buffer = Vec::new();
                let mut chunk = [0u8; 4096];
                loop {
                    let Ok(read) = socket.read(&mut chunk).await else {
                        return;
                    };
                    if read == 0 {
                        return;
                    }
                    buffer.extend_from_slice(&chunk[..read]);
                    let text = String::from_utf8_lossy(&buffer).into_owned();
                    let Some(header_end) = text.find("\r\n\r\n") else {
                        continue;
                    };
                    let content_length = text
                        .get(..header_end)
                        .unwrap_or("")
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            if name.eq_ignore_ascii_case("content-length") {
                                value.trim().parse::<usize>().ok()
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    let body_start = header_end + 4;
                    if buffer.len() < body_start + content_length {
                        continue;
                    }
                    let body = buffer
                        .get(body_start..body_start + content_length)
                        .unwrap_or(&[]);
                    let request: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
                    let response = serde_json::to_vec(&respond(&request)).expect("encode");
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        response.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(&response).await;
                    let _ = socket.flush().await;
                    return;
                }
            });
        }
    });
    format!("http://127.0.0.1:{}", address.port())
}

fn account_set() -> AccountSetConfig {
    let positions = vec![
        PositionConfig {
            key: POOL_KEY,
            expected_owner: POOL_OWNER,
            inline_len: 4,
            admitted_data_lens: vec![4],
        },
        PositionConfig {
            key: PROGRAMDATA_KEY,
            expected_owner: LOADER_V3_PROGRAM_ID,
            inline_len: 45,
            admitted_data_lens: vec![u32::try_from(PROGRAMDATA_LEN).unwrap()],
        },
    ];
    let entries = positions
        .iter()
        .map(
            |position| dclutch_relay_contract::release::AccountSetEntryV1 {
                key: position.key,
                expected_owner: position.expected_owner,
                inline_len: position.inline_len,
            },
        )
        .collect::<Vec<_>>();
    let account_set_id = derive_account_set_id(CLUSTER, FAMILY, &entries).expect("derive");
    AccountSetConfig {
        name: "mock-set".to_owned(),
        relay_family_id: FAMILY,
        decoding_rules_id: RULES,
        positions,
        account_set_id,
    }
}

#[tokio::test]
async fn one_cycle_observes_signs_folds_and_writes_verifiable_artifacts() {
    let url = spawn_mock().await;
    let rpc = RpcClient::new(&url, Duration::from_secs(5), None).expect("client");
    rpc.require_expected_genesis(CLUSTER)
        .await
        .expect("the mock reports the pinned genesis hash");

    let dir = tempfile::tempdir().expect("tempdir");
    let key_path = dir.path().join("keys/attestation.json");
    let public = generate_keypair_file(&key_path, None).expect("keygen");
    let signer = AttestationSigner::load(&key_path, None).expect("load");
    assert_eq!(signer.public_key(), public);

    let set = account_set();
    // The minimum admitted page width, so the 1045-byte body needs three pages
    // and the multi-page path is the one under test.
    let mut watcher = SetWatcher::new(set.clone(), CLUSTER, 448);
    let cycle = watcher.observe(&rpc, &[], &signer).await.expect("observe");

    assert_eq!(cycle.observed_slot, OBSERVED_SLOT);
    assert_eq!(cycle.set_count, 2);
    assert_eq!(cycle.positions.len(), 2);
    assert_eq!(cycle.account_set_id, set.account_set_id);

    // Position 0 is fully inline: its tail digest is the empty-string digest,
    // and no page was read for it.
    let pool = cycle.positions.first().expect("pool");
    assert_eq!(pool.inline, pool_bytes());
    assert_eq!(pool.data_len, 4);
    assert_eq!(pool.tail_digest, SHA256_EMPTY_DIGEST);
    assert_eq!(pool.tail_digest_source, TailDigestSource::FullyInline);

    // Position 1 is a Loader V3 ProgramData: 45 bytes inline, the rest digested.
    let programdata = cycle.positions.get(1).expect("programdata");
    let full = programdata_bytes();
    assert_eq!(programdata.inline, full.get(..45).expect("prefix").to_vec());
    assert_eq!(
        programdata.data_len,
        u32::try_from(PROGRAMDATA_LEN).unwrap()
    );
    assert_eq!(
        programdata.tail_digest,
        sha256(full.get(45..).expect("tail")),
        "the tail digest must be SHA-256 over data[45..]"
    );
    match programdata.tail_digest_source {
        TailDigestSource::Paged { pages, bytes } => {
            assert_eq!(pages, 3, "1045 bytes at 448 per page is three pages");
            assert_eq!(bytes, (PROGRAMDATA_LEN - 45) as u64);
        }
        other => panic!("expected a paged tail digest, got {other:?}"),
    }

    // Every attestation decodes back through the wire crate's own decoder and
    // verifies against the generated key.
    let mut fold = SetDigestFold::seed(set.account_set_id, OBSERVED_SLOT).expect("seed");
    for position in &cycle.positions {
        let decoded =
            AttestationMessageV1::decode(&position.message_bytes).expect("attestation decodes");
        assert_eq!(decoded.observed_cluster_id(), CLUSTER);
        assert_eq!(decoded.relay_family_id(), FAMILY);
        assert_eq!(decoded.decoding_rules_id(), RULES);
        assert_eq!(decoded.account_set_id(), set.account_set_id);
        assert_eq!(decoded.observed_slot(), OBSERVED_SLOT);
        assert_eq!(decoded.set_index(), position.set_index);
        assert_eq!(decoded.set_count(), 2);
        assert_eq!(decoded.body().key(), position.key);
        assert_eq!(decoded.body().data_len(), position.data_len);
        assert_eq!(decoded.body().tail_digest(), position.tail_digest);
        assert!(
            signer.verify(&position.message_bytes, &position.signature),
            "the attestation signature must verify against exactly the encoded bytes"
        );
        assert!(
            !signer.verify(b"different bytes", &position.signature),
            "the signature must not verify over other bytes"
        );
        fold.absorb(&position.body_bytes);
    }
    assert_eq!(
        fold.digest(),
        cycle.set_digest,
        "the daemon's fold and an independent fold over the same bodies must agree"
    );

    let seal = ObservationSetSealV1::decode(&cycle.seal_bytes).expect("seal decodes");
    assert_eq!(seal.set_digest(), cycle.set_digest);
    assert_eq!(seal.observed_slot(), OBSERVED_SLOT);
    assert_eq!(seal.set_count(), 2);
    assert_eq!(seal.account_set_id(), set.account_set_id);
    assert!(signer.verify(&cycle.seal_bytes, &cycle.seal_signature));
    assert_eq!(cycle.seal_bytes.len(), RELAYED_SEAL_BYTES);

    // Artifacts and the publication log.
    let publication = PublicationLog::open(dir.path()).expect("log");
    for position in &cycle.positions {
        publication
            .record(
                MessageKind::Attestation,
                &cycle.set_name,
                &cycle.account_set_id,
                cycle.observed_slot,
                Some(position.set_index),
                &position.message_bytes,
                &cycle.signer,
                &position.signature,
            )
            .expect("record");
    }
    publication
        .record(
            MessageKind::Seal,
            &cycle.set_name,
            &cycle.account_set_id,
            cycle.observed_slot,
            None,
            &cycle.seal_bytes,
            &cycle.signer,
            &cycle.seal_signature,
        )
        .expect("record");

    let written = ArtifactWriter::new(dir.path())
        .write_cycle(&cycle)
        .expect("artifacts");
    assert!(written.ends_with(Path::new("mock-set/slot-423941138")));

    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(written.join("manifest.json")).expect("manifest"),
    )
    .expect("json");
    assert_eq!(manifest["set_count"], 2);
    assert_eq!(manifest["rpc"]["paged_body_reads"], 3);
    assert_eq!(
        manifest["attestation_signer_pubkey_base58"],
        base58(&public)
    );

    // The bytes on disk are the exact bytes that were signed.
    for position in &cycle.positions {
        let on_disk =
            std::fs::read(written.join(format!("attestation.{}.bin", position.set_index)))
                .expect("bin");
        assert_eq!(on_disk, position.message_bytes);
        let signature =
            std::fs::read(written.join(format!("attestation.{}.sig", position.set_index)))
                .expect("sig");
        assert_eq!(signature, position.signature.to_vec());
        let signature: [u8; 64] = signature.try_into().expect("64 bytes");
        assert!(signer.verify(&on_disk, &signature));
    }

    let publication_lines =
        std::fs::read_to_string(dir.path().join("publication_log.jsonl")).expect("publication");
    assert_eq!(publication_lines.lines().count(), 3);

    // The raw RPC response is kept verbatim, which is what makes the artifact
    // checkable against the cluster by a third party.
    let raw: Value = serde_json::from_str(
        &std::fs::read_to_string(written.join("rpc_get_multiple_accounts.json")).expect("raw"),
    )
    .expect("json");
    assert_eq!(raw["context"]["slot"], OBSERVED_SLOT);
}

#[tokio::test]
async fn a_cluster_that_is_not_the_pinned_cluster_refuses_before_anything_is_signed() {
    let url = spawn_mock().await;
    let rpc = RpcClient::new(&url, Duration::from_secs(5), None).expect("client");
    let error = rpc
        .require_expected_genesis(dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1)
        .await
        .expect_err("the mock is mainnet, so a devnet pin must refuse");
    assert!(
        matches!(
            error,
            dclutch_relayer::error::RelayerError::GenesisMismatch { .. }
        ),
        "{error:?}"
    );
}
