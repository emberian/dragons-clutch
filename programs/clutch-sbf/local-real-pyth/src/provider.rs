//! Exact provider fixtures plus a fresh deterministic synthetic VAA.
//!
//! The signing construction is derived from Pyth's Apache-2.0
//! `pythnet_sdk::test_utils` at pyth-crosschain commit
//! `f50a3faf9fc5a223a22889799b2f778900f186b3`. Unlike the upstream helper,
//! which randomly chooses a valid quorum, this laboratory always selects
//! guardian indices 0 through 12 so output is reproducible for its inputs.

use borsh::to_vec;
use clutch_sbf::{
    pyth_receiver::config_byte_digest,
    source_identity::real_pyth_lab,
    source_v2::fixtures::{programdata_body, receiver_program_body},
};
use libsecp256k1::{Message as SecpMessage, RecoveryId, SecretKey, Signature};
use pyth_solana_receiver_sdk::PostUpdateParams;
use pythnet_sdk::{
    messages::{Message, PriceFeedMessage},
    test_utils::create_accumulator_message,
    wire::v1::{AccumulatorUpdateData, MerklePriceUpdate, Proof},
};
use serde_wormhole::RawMessage;
use solana_address::Address;
use std::{fs, path::Path};
use wormhole_sdk::vaa::{Body, Header, Signature as VaaSignature};
use wormhole_sdk::Vaa;

pub const PRICE: i64 = 100_000_000;
pub const CONFIDENCE: u64 = 6_357;
pub const EXPONENT: i32 = -8;
pub const FEED_ID: [u8; 32] = [0x2a; 32];

const RECEIVER_PROGRAM_HASH: &str =
    "ef37dd1cee22d731902a8c04ed2e13136a2b8aa7068d9db3aff2ed1ec7b634e5";
const RECEIVER_PROGRAMDATA_HASH: &str =
    "7122abc6b5e78d30bf88c869cb5d8783adaf897369d04eca827d3af8ffe18e5d";
const ROUTER_PROGRAM_HASH: &str =
    "1ee590ae23d5ecbf775aba910f06a993dee8f77bfd7028790dbd349651c8034b";
const ROUTER_PROGRAMDATA_HASH: &str =
    "f26f4b53b0f980455886116f500fa74ba475e51b1acb7f486b18afa9d73d948f";

pub struct ProviderAccount {
    pub role: &'static str,
    pub address: Address,
    pub data: Vec<u8>,
    pub executable: bool,
    pub expected_hash: &'static str,
}

pub struct Observation {
    pub vaa: Vec<u8>,
    pub update: MerklePriceUpdate,
    pub post_data: Vec<u8>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn sha256(bytes: &[u8]) -> String {
    hex(&config_byte_digest(bytes))
}

pub fn fixture(name: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let expected = match name {
        "receiver.so" => "c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64",
        "router.so" => "f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb",
        "router-initialize.data" => {
            "3667940a4428a8f2411a0ff11157ecc4ba1076c3c61273a108da6405c51e0b0b"
        }
        "receiver-initialize.data" => {
            "d9c80906af92f99a0c8441f4463186056b1c12cb990999acfa198a46ec62729f"
        }
        "receiver-config.account" => {
            "05038cf707afceac3df1aae735b096344ad639506b00f1db0ac1c084d6b645aa"
        }
        "receiver-post-update.data" => {
            "3bf9188bd6183155ea30738c3ab9da706ea7013bf5a7887a531e90b9bea85e1d"
        }
        other => return Err(format!("no executable fixture hash pin for {other}").into()),
    };
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../svm-tests/tests/fixtures/real-pyth-local");
    let bytes = fs::read(root.join(name))?;
    let actual = sha256(&bytes);
    if actual != expected {
        return Err(format!("fixture {name} hash {actual} differs from {expected}").into());
    }
    Ok(bytes)
}

pub fn deployment_accounts() -> Result<Vec<ProviderAccount>, Box<dyn std::error::Error>> {
    let receiver_program = receiver_program_body(real_pyth_lab::RECEIVER_PROGRAMDATA);
    let receiver_programdata = programdata_body(
        real_pyth_lab::RECEIVER_DEPLOYMENT_SLOT,
        Some(real_pyth_lab::UPGRADE_AUTHORITY),
        [0; 32],
        &fixture("receiver.so")?,
    );
    let router_program = receiver_program_body(real_pyth_lab::ROUTER_PROGRAMDATA);
    let router_programdata = programdata_body(
        real_pyth_lab::ROUTER_DEPLOYMENT_SLOT,
        Some(real_pyth_lab::UPGRADE_AUTHORITY),
        [0; 32],
        &fixture("router.so")?,
    );
    let accounts = vec![
        ProviderAccount {
            role: "receiver-program",
            address: Address::new_from_array(real_pyth_lab::RECEIVER_PROGRAM),
            data: receiver_program,
            executable: true,
            expected_hash: RECEIVER_PROGRAM_HASH,
        },
        ProviderAccount {
            role: "receiver-programdata",
            address: Address::new_from_array(real_pyth_lab::RECEIVER_PROGRAMDATA),
            data: receiver_programdata,
            executable: false,
            expected_hash: RECEIVER_PROGRAMDATA_HASH,
        },
        ProviderAccount {
            role: "router-program",
            address: Address::new_from_array(real_pyth_lab::ROUTER_PROGRAM),
            data: router_program,
            executable: true,
            expected_hash: ROUTER_PROGRAM_HASH,
        },
        ProviderAccount {
            role: "router-programdata",
            address: Address::new_from_array(real_pyth_lab::ROUTER_PROGRAMDATA),
            data: router_programdata,
            executable: false,
            expected_hash: ROUTER_PROGRAMDATA_HASH,
        },
    ];
    for account in &accounts {
        let actual = sha256(&account.data);
        if actual != account.expected_hash {
            return Err(format!(
                "{} complete body hash {actual} differs from {}",
                account.role, account.expected_hash
            )
            .into());
        }
    }
    Ok(accounts)
}

fn guardians() -> Vec<SecretKey> {
    (1_u8..=19)
        .map(|index| {
            let mut bytes = [0_u8; 32];
            bytes[0] = index;
            SecretKey::parse(&bytes).expect("upstream deterministic guardian key")
        })
        .collect()
}

fn deterministic_vaa(
    random_subset_vaa: &[u8],
    publish_time: i64,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let (_header, mut body): (Header, Body<&RawMessage>) =
        serde_wormhole::from_slice(random_subset_vaa)?;
    body.timestamp = u32::try_from(publish_time)
        .map_err(|_| format!("publish time {publish_time} does not fit Wormhole u32 timestamp"))?;
    let digest = SecpMessage::parse_slice(&body.digest()?.secp256k_hash)?;
    let signatures: Vec<(Signature, RecoveryId)> = guardians()
        .iter()
        .take(13)
        .map(|guardian| libsecp256k1::sign(&digest, guardian))
        .collect();
    let vaa_signatures = signatures
        .iter()
        .enumerate()
        .map(|(index, (signature, recovery))| {
            let mut bytes = [0_u8; 65];
            bytes[..64].copy_from_slice(&signature.serialize());
            bytes[64] = recovery.serialize();
            VaaSignature {
                index: u8::try_from(index).expect("13 guardians fit in u8"),
                signature: bytes,
            }
        })
        .collect();
    let header = Header {
        version: 1,
        guardian_set_index: 0,
        signatures: vaa_signatures,
    };
    let vaa: Vaa<&RawMessage> = (header, body).into();
    Ok(serde_wormhole::to_vec(&vaa)?)
}

pub fn observation_for_feed(
    publish_time: i64,
    feed_id: [u8; 32],
) -> Result<Observation, Box<dyn std::error::Error>> {
    let message = Message::PriceFeedMessage(PriceFeedMessage {
        feed_id,
        price: PRICE,
        conf: CONFIDENCE,
        exponent: EXPONENT,
        publish_time,
        prev_publish_time: publish_time - 1,
        ema_price: PRICE - 1_000,
        ema_conf: 6_400,
    });
    let accumulator = create_accumulator_message(&[&message], &[&message], false, false, None);
    let parsed = AccumulatorUpdateData::try_from_slice(&accumulator)?;
    let Proof::WormholeMerkle { vaa, mut updates } = parsed.proof;
    if updates.len() != 1 {
        return Err(format!("generator returned {} updates", updates.len()).into());
    }
    let random_vaa: Vec<u8> = vaa.into();
    let vaa = deterministic_vaa(&random_vaa, publish_time)?;
    let update = updates.remove(0);
    let mut post_data = real_pyth_lab::POST_UPDATE_DISCRIMINATOR.to_vec();
    post_data.extend_from_slice(&to_vec(&PostUpdateParams {
        merkle_price_update: update.clone(),
        treasury_id: 0,
    })?);
    Ok(Observation {
        vaa,
        update,
        post_data,
    })
}

pub fn observation(publish_time: i64) -> Result<Observation, Box<dyn std::error::Error>> {
    observation_for_feed(publish_time, FEED_ID)
}

#[cfg(test)]
fn guardian_addresses() -> Vec<[u8; 20]> {
    use libsecp256k1::PublicKey;
    use pythnet_sdk::hashers::{keccak256::Keccak256, Hasher};

    guardians()
        .iter()
        .map(|guardian| {
            let serialized = PublicKey::from_secret_key(guardian).serialize();
            let mut out = [0_u8; 20];
            out.copy_from_slice(&Keccak256::hashv(&[&serialized[1..]])[12..]);
            out
        })
        .collect()
}

#[cfg(test)]
fn default_data_source() -> (u16, [u8; 32]) {
    use pythnet_sdk::test_utils::DEFAULT_DATA_SOURCE;

    (
        DEFAULT_DATA_SOURCE.chain.into(),
        DEFAULT_DATA_SOURCE.address.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deployment_bodies_match_all_four_capture_hashes() {
        deployment_accounts().unwrap();
    }

    #[test]
    fn fresh_generator_is_reproducible_for_a_named_time() {
        let left = observation(1_800_000_000).unwrap();
        let right = observation(1_800_000_000).unwrap();
        assert_eq!(left.vaa, right.vaa);
        assert_eq!(left.post_data, right.post_data);
        assert_eq!(left.vaa.len(), 952);
        assert_eq!(left.post_data.len(), 102);
        let (_header, body): (Header, Body<&RawMessage>) =
            serde_wormhole::from_slice(&left.vaa).unwrap();
        assert_eq!(body.timestamp, 1_800_000_000);
    }

    #[test]
    fn feed_identity_changes_the_signed_update_and_post_body() {
        use byteorder::BE;

        let correct = observation(1_800_000_000).unwrap();
        let wrong = observation_for_feed(1_800_000_000, [0x2b; 32]).unwrap();
        assert_ne!(correct.vaa, wrong.vaa);
        assert_ne!(correct.update.message, wrong.update.message);
        assert_ne!(correct.post_data, wrong.post_data);
        let decoded: Message =
            pythnet_sdk::wire::from_slice::<BE, _>(wrong.update.message.as_ref()).unwrap();
        assert_eq!(decoded.feed_id(), [0x2b; 32]);
    }

    #[test]
    fn fixed_capture_time_reproduces_the_post_update_abi_bytes() {
        let generated = observation(1_787_431_680).unwrap();
        assert_eq!(
            generated.post_data,
            fixture("receiver-post-update.data").unwrap()
        );
    }

    #[test]
    fn public_guardians_match_the_captured_router_initializer() {
        let expected = guardian_addresses();
        assert_eq!(expected.len(), 19);
        let initializer = fixture("router-initialize.data").unwrap();
        assert_eq!(initializer.len(), 1 + 4 + 8 + 4 + 19 * 20);
        assert_eq!(initializer[0], 0);
        assert_eq!(
            u32::from_le_bytes(initializer[1..5].try_into().unwrap()),
            86_400
        );
        assert_eq!(
            u64::from_le_bytes(initializer[5..13].try_into().unwrap()),
            0
        );
        assert_eq!(
            u32::from_le_bytes(initializer[13..17].try_into().unwrap()),
            19
        );
        let decoded: Vec<[u8; 20]> = initializer[17..]
            .chunks_exact(20)
            .map(|bytes| bytes.try_into().unwrap())
            .collect();
        assert_eq!(decoded, expected);
        let (_chain, emitter) = default_data_source();
        assert_eq!(emitter, [1; 32]);
    }
}
