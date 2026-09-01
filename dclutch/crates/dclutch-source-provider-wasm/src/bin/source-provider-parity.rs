//! Native side of Source-provider native/WASM parity checks.

use std::io::{self, Read};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use dclutch_pyth_svm::devnet_release_v1;
use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;
use dclutch_resolution_codec::{
    PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
    ProviderSubmitRequestV3, ProviderUpdateLifecycleV3, ResolutionCertificateKindV2,
    ResolutionCertificateV2,
};
use serde_json::json;
use solana_hash::Hash;
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::system_program;

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--fixture") => {
            let input = fixture();
            let output =
                dclutch_source_provider_wasm::plan_provider_reclaim_json_v1(input.as_bytes())
                    .expect("checked parity fixture");
            println!("{}", json!({ "input": input, "output": output }));
            return;
        }
        Some("--submit-fresh-fixture") => {
            let input = submit_fresh_fixture();
            let output = dclutch_source_provider_wasm::derive_provider_submit_fresh_json_v1(
                input.as_bytes(),
            )
            .expect("checked submit-fresh fixture");
            println!("{}", json!({ "input": input, "output": output }));
            return;
        }
        Some("--submit-poststate-fixture") => {
            let input = submit_poststate_fixture();
            let output = dclutch_source_provider_wasm::verify_provider_submit_poststate_json_v1(
                input.as_bytes(),
            )
            .expect("checked submit-poststate fixture");
            println!("{}", json!({ "input": input, "output": output }));
            return;
        }
        _ => {}
    }
    let mut source = Vec::new();
    if let Err(error) = io::stdin().read_to_end(&mut source) {
        eprintln!("read Source-provider parity input: {error}");
        std::process::exit(1);
    }
    match dclutch_source_provider_wasm::plan_provider_reclaim_json_v1(&source) {
        Ok(output) => println!("{output}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn submit_fresh_fixture() -> String {
    json!({
        "format": "dclutch-source-provider-submit-fresh-input-v1",
        "market": key(50).to_string(),
        "sourceState": key(51).to_string(),
        "updateAccount": key(52).to_string(),
        "resolutionProgram": key(53).to_string(),
    })
    .to_string()
}

fn submit_poststate_fixture() -> String {
    let update_data = vec![1, 2, 3, 4];
    let request = ProviderSubmitRequestV3 {
        generation: 7,
        reclaim_after_unix_seconds: 1_900_000_000,
        market: key(60).to_bytes(),
        source_state: key(61).to_bytes(),
        lifecycle: key(62).to_bytes(),
        source_material: key(63).to_bytes(),
        provider_release: key(64).to_bytes(),
        update_account: key(65).to_bytes(),
        provider_submitter: key(66).to_bytes(),
        refund_recipient: key(67).to_bytes(),
        release_set: key(68).to_bytes(),
        registry_program: key(69).to_bytes(),
        encoded_vaa: key(70).to_bytes(),
        post_body_digest: key(71).to_bytes(),
    };
    let authority = key(72);
    let resolution = key(73);
    let receiver = key(74);
    let lifecycle = ProviderUpdateLifecycleV3::submitted(
        request,
        1,
        authority.to_bytes(),
        request.registry_program,
        hash(&update_data).to_bytes(),
        1_800_000_000,
        90,
        2_000,
        1,
    )
    .expect("submitted lifecycle")
    .to_bytes()
    .expect("lifecycle bytes");
    json!({
        "format": "dclutch-source-provider-submit-poststate-input-v1",
        "expectation": {
            "lifecycle": key(62).to_string(),
            "updateAccount": key(65).to_string(),
            "updateAuthority": authority.to_string(),
            "resolutionProgram": resolution.to_string(),
            "receiverProgram": receiver.to_string(),
            "submitRequestBase64": STANDARD.encode(request.to_bytes().expect("request")),
        },
        "lifecycle": account(key(62), resolution, 1_000, &lifecycle),
        "update": account(key(65), receiver, 2_000, &update_data),
    })
    .to_string()
}

fn key(byte: u8) -> Pubkey {
    Pubkey::new_from_array([byte; 32])
}

fn account(address: Pubkey, owner: Pubkey, lamports: u64, data: &[u8]) -> serde_json::Value {
    json!({
        "address": address.to_string(),
        "owner": owner.to_string(),
        "lamports": lamports.to_string(),
        "executable": false,
        "dataBase64": STANDARD.encode(data),
    })
}

fn fixture() -> String {
    let resolution = key(30);
    let registry = key(31);
    let update = key(32);
    let (lifecycle_key, bump) = Pubkey::find_program_address(
        &[PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, update.as_ref()],
        &resolution,
    );
    let release = devnet_release_v1().expect("pinned devnet release");
    let release_bytes = release.to_bytes();
    let release_digest = hash(&release_bytes).to_bytes();
    let release_key = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
            &release_digest,
        ],
        &registry,
    )
    .0;
    let request = ProviderSubmitRequestV3 {
        generation: 7,
        reclaim_after_unix_seconds: 1_800_000_100,
        market: key(1).to_bytes(),
        source_state: key(2).to_bytes(),
        lifecycle: lifecycle_key.to_bytes(),
        source_material: key(3).to_bytes(),
        provider_release: release_digest,
        update_account: update.to_bytes(),
        provider_submitter: key(4).to_bytes(),
        refund_recipient: key(5).to_bytes(),
        release_set: key(6).to_bytes(),
        registry_program: registry.to_bytes(),
        encoded_vaa: key(7).to_bytes(),
        post_body_digest: key(8).to_bytes(),
    };
    let update_data = vec![1, 2, 3, 4];
    let mut lifecycle = ProviderUpdateLifecycleV3::submitted(
        request,
        bump,
        key(9).to_bytes(),
        registry.to_bytes(),
        hash(&update_data).to_bytes(),
        1_800_000_000,
        89,
        2_000_000,
        1,
    )
    .expect("submitted lifecycle");
    lifecycle
        .consume(3, key(11).to_bytes(), key(12).to_bytes())
        .expect("consumed lifecycle");
    let lifecycle_bytes = lifecycle.to_bytes().expect("lifecycle bytes");
    let certificate = ResolutionCertificateV2 {
        kind: ResolutionCertificateKindV2::ResolutionSuccess,
        market: request.market,
        route: request.provider_release,
        source_material: request.source_material,
        product_record_digest: key(14).to_bytes(),
        provider_evidence: key(11).to_bytes(),
        funding_allocation: [0; 32],
        receipt_account: key(12).to_bytes(),
        generation: request.generation,
        attempt_index: 0,
        schedule_index: 0,
        selector: 0,
        work_paid: 0,
        funding_remaining: 0,
        result_numerator: 1,
        result_denominator: 1,
        observed_at: 1_800_000_000,
    }
    .to_bytes()
    .expect("certificate bytes");
    json!({
        "format": "dclutch-source-provider-reclaim-input-v1",
        "observedSlot": "90",
        "unixTimestamp": "1800000200",
        "recentBlockhash": Hash::new_from_array([13; 32]).to_string(),
        "lifecycle": account(lifecycle_key, resolution, 1_500_000, &lifecycle_bytes),
        "pythRelease": account(release_key, registry, 1_000_000, &release_bytes),
        "update": account(update, Pubkey::new_from_array(release.receiver_program()), 2_000_000, &update_data),
        "updateAuthority": account(key(9), system_program::ID, 0, &[]),
        "refundRecipient": account(key(5), system_program::ID, 100, &[]),
        "certificate": account(key(12), resolution, 1_000_000, &certificate),
        "deployment": {
            "payer": key(40).to_string(),
            "resolver": key(41).to_string(),
            "registryProgramdata": key(42).to_string(),
            "resolutionProgram": resolution.to_string(),
            "resolutionProgramdata": key(43).to_string(),
        },
        "lookupTable": null,
    })
    .to_string()
}
