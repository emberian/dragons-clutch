//! Strict detached Ed25519 verification for accepted orchestration envelopes.
//!
//! This binary deliberately has no signing or key-file surface. It exposes the
//! relayer's existing `verify_detached` semantic owner as a small, hash-pinnable
//! process boundary for callers that must verify public authorization bytes.

use std::process::ExitCode;

use base64::Engine as _;
use clap::Parser;
use sha2::{Digest as _, Sha256};

const RESULT_SCHEMA: &str = "dclutch-ed25519-verification-v1";

#[derive(Parser)]
#[command(name = "dclutch-verify-ed25519")]
#[command(about = "Verify one detached Ed25519 signature; never sign")]
struct Arguments {
    /// Base58-encoded 32-byte Ed25519 public key.
    #[arg(long)]
    public_key_base58: String,
    /// Base58-encoded 64-byte detached Ed25519 signature.
    #[arg(long)]
    signature_base58: String,
    /// Canonical message bytes encoded as standard padded base64.
    #[arg(long)]
    message_base64: String,
    /// Lowercase SHA-256 of the decoded message bytes.
    #[arg(long)]
    message_sha256: String,
}

fn decode_base58<const N: usize>(value: &str) -> Option<[u8; N]> {
    let mut output = [0_u8; N];
    let written = bs58::decode(value).onto(&mut output).ok()?;
    (written == N).then_some(output)
}

fn verify(arguments: &Arguments) -> bool {
    if arguments.message_sha256.len() != 64
        || !arguments
            .message_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return false;
    }
    let Ok(message) = base64::engine::general_purpose::STANDARD.decode(&arguments.message_base64)
    else {
        return false;
    };
    if base64::engine::general_purpose::STANDARD.encode(&message) != arguments.message_base64
        || hex::encode(Sha256::digest(&message)) != arguments.message_sha256
    {
        return false;
    }
    let Some(public_key) = decode_base58::<32>(&arguments.public_key_base58) else {
        return false;
    };
    let Some(signature) = decode_base58::<64>(&arguments.signature_base58) else {
        return false;
    };
    dclutch_relayer::keys::verify_detached(&public_key, &message, &signature)
}

fn main() -> ExitCode {
    let arguments = Arguments::parse();
    if !verify(&arguments) {
        eprintln!("Ed25519 verification refused");
        return ExitCode::from(2);
    }
    println!(
        "{{\"schema\":\"{RESULT_SCHEMA}\",\"messageSha256\":\"{}\",\"publicKeyBase58\":\"{}\",\"signatureBase58\":\"{}\",\"verified\":true}}",
        arguments.message_sha256, arguments.public_key_base58, arguments.signature_base58,
    );
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 8032 test vector 1: an empty message. The vector is public evidence,
    // not a key read and this binary has no signing operation.
    const PUBLIC_KEY_HEX: &str = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
    const SIGNATURE_HEX: &str = concat!(
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155",
        "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
    );

    fn vector() -> Arguments {
        let public_key = hex::decode(PUBLIC_KEY_HEX).expect("constant public vector");
        let signature = hex::decode(SIGNATURE_HEX).expect("constant signature vector");
        Arguments {
            public_key_base58: bs58::encode(public_key).into_string(),
            signature_base58: bs58::encode(signature).into_string(),
            message_base64: String::new(),
            message_sha256: hex::encode(Sha256::digest([])),
        }
    }

    #[test]
    fn accepts_rfc_8032_vector_and_refuses_forgery() {
        let accepted = vector();
        assert!(verify(&accepted));

        let mut forged = vector();
        forged.message_base64 = "AA==".to_owned();
        forged.message_sha256 = hex::encode(Sha256::digest([0_u8]));
        assert!(!verify(&forged));
    }

    #[test]
    fn refuses_digest_or_noncanonical_base64_substitution() {
        let mut wrong_digest = vector();
        wrong_digest.message_sha256 = "0".repeat(64);
        assert!(!verify(&wrong_digest));

        let mut noncanonical = vector();
        noncanonical.message_base64 = "====".to_owned();
        assert!(!verify(&noncanonical));
    }
}
