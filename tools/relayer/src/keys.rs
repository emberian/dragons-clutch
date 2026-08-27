//! Signing keys, and the paths this service refuses to open.
//!
//! Two rules govern everything here, and they are the point rather than a
//! precaution:
//!
//! 1. **The relayer never looks for a wallet.** There is no default path, no
//!    scan of `~/.config/solana`, no `id.json` fallback. The only paths ever
//!    opened are the ones the operator wrote into the config file.
//! 2. **A path that looks like a real wallet store is refused even when named
//!    explicitly**, because the most likely way this service ever touches a
//!    funded key is a copy-pasted config, not a decision.
//!
//! The attestation key and the fee-payer key are distinct fields with distinct
//! lifetimes (§4.11): the fee payer is hot and replaceable, the attestation key
//! *is* the provider release identity and in the hardened profile lives behind
//! a separate process or HSM boundary that receives message bytes and returns
//! signatures.  This module is the soft profile; the boundary it would sit
//! behind is [`AttestationSigner`], which is the only thing the observation
//! loop is handed.

use std::path::{Component, Path, PathBuf};

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};

use crate::error::{RelayerError, Result};
use crate::id32::{ID_BYTES, base58};

/// Exact width of a Solana-convention keypair file's byte array.
const KEYPAIR_FILE_BYTES: usize = 64;

/// Expand a single leading `~` to the supplied home directory.
///
/// Done before the safety check rather than after, so `~/.config/solana/id.json`
/// is refused for the same reason its expansion is.
pub fn expand_tilde(path: &Path, home: Option<&Path>) -> PathBuf {
    let Some(home) = home else {
        return path.to_path_buf();
    };
    let text = path.as_os_str().to_string_lossy();
    if text == "~" {
        return home.to_path_buf();
    }
    match text.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => path.to_path_buf(),
    }
}

/// Refuse a keypair path that names a user's real key store.
///
/// The rules are lexical on purpose: they must hold for a path that does not
/// exist yet (`keygen`) exactly as they hold for one that does, and a
/// canonicalization step would need to open the very thing being refused.
pub fn require_safe_keypair_path(path: &Path, home: Option<&Path>) -> Result<()> {
    let refuse = |reason: &str| {
        Err(RelayerError::UnsafeKeypairPath {
            path: path.to_path_buf(),
            reason: reason.to_owned(),
        })
    };

    let components: Vec<String> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();

    if components.iter().any(|name| name == ".ssh") {
        return refuse("the path traverses a .ssh directory");
    }
    if components
        .windows(2)
        .any(|pair| matches!(pair, [first, second] if first == ".config" && second == "solana"))
    {
        return refuse("the path traverses .config/solana, the default Solana wallet store");
    }
    if let Some(home) = home {
        if path.starts_with(home.join(".config")) {
            return refuse("the path is inside the user's ~/.config dotfiles");
        }
        if path.starts_with(home.join(".ssh")) {
            return refuse("the path is inside the user's ~/.ssh dotfiles");
        }
    }
    Ok(())
}

/// One loaded Ed25519 signing key, and the only handle the loop is given.
pub struct AttestationSigner {
    key: SigningKey,
    source: PathBuf,
}

/// Never prints key material.  A `Debug` that could leak a seed into a log
/// line is a worse failure than no `Debug` at all.
impl core::fmt::Debug for AttestationSigner {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("AttestationSigner")
            .field("public_key", &self.public_key_base58())
            .field("source", &self.source)
            .finish()
    }
}

impl AttestationSigner {
    /// Load a signing key from a path the operator named in config.
    ///
    /// `home` is the directory the safety rules are measured against; passing
    /// `None` disables only the home-relative rules, never the lexical ones.
    pub fn load(path: &Path, home: Option<&Path>) -> Result<Self> {
        require_safe_keypair_path(path, home)?;
        let text =
            std::fs::read_to_string(path).map_err(|source| RelayerError::io(path, source))?;
        let raw: Vec<u8> =
            serde_json::from_str(&text).map_err(|source| RelayerError::MalformedKeypair {
                path: path.to_path_buf(),
                reason: format!("expected a JSON array of {KEYPAIR_FILE_BYTES} bytes: {source}"),
            })?;
        Self::from_file_bytes(&raw, path)
    }

    /// Build a signer from the exact 64-byte Solana-convention array.
    ///
    /// The trailing 32 bytes are the claimed public key.  They are *checked*
    /// against the key derived from the seed rather than trusted: a file whose
    /// halves disagree is a corrupted or hand-edited file, and signing with it
    /// would produce attestations attributed to a key that cannot verify them.
    pub fn from_file_bytes(raw: &[u8], source: &Path) -> Result<Self> {
        let refuse = |reason: String| RelayerError::MalformedKeypair {
            path: source.to_path_buf(),
            reason,
        };
        if raw.len() != KEYPAIR_FILE_BYTES {
            return Err(refuse(format!(
                "expected {KEYPAIR_FILE_BYTES} bytes, found {}",
                raw.len()
            )));
        }
        let seed: [u8; ID_BYTES] = raw
            .get(..ID_BYTES)
            .and_then(|slice| slice.try_into().ok())
            .ok_or_else(|| refuse("seed half is not 32 bytes".to_owned()))?;
        let claimed: [u8; ID_BYTES] = raw
            .get(ID_BYTES..)
            .and_then(|slice| slice.try_into().ok())
            .ok_or_else(|| refuse("public half is not 32 bytes".to_owned()))?;
        let key = SigningKey::from_bytes(&seed);
        if key.verifying_key().to_bytes() != claimed {
            return Err(refuse(
                "the file's public half does not match the key derived from its seed".to_owned(),
            ));
        }
        Ok(Self {
            key,
            source: source.to_path_buf(),
        })
    }

    /// The public key that identifies this signer in the relayer key set.
    pub fn public_key(&self) -> [u8; ID_BYTES] {
        self.key.verifying_key().to_bytes()
    }

    /// The public key rendered base58.
    pub fn public_key_base58(&self) -> String {
        base58(&self.public_key())
    }

    /// The path this key was loaded from, for diagnostics.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// Sign exactly the bytes handed in.
    ///
    /// This is the whole HSM-shaped surface: message bytes in, 64 signature
    /// bytes out.  Nothing here inspects, reformats, or re-derives the message,
    /// because a signer that understands its message is a signer that can be
    /// argued into signing a different one.
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.key.sign(message).to_bytes()
    }

    /// Verify a signature this signer produced, used by tests and by the
    /// artifact writer's self-check.
    pub fn verify(&self, message: &[u8], signature: &[u8; 64]) -> bool {
        verify_detached(&self.public_key(), message, signature)
    }
}

/// Verify one detached Ed25519 signature.
pub fn verify_detached(public_key: &[u8; ID_BYTES], message: &[u8], signature: &[u8; 64]) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(public_key) else {
        return false;
    };
    key.verify_strict(message, &ed25519_dalek::Signature::from_bytes(signature))
        .is_ok()
}

/// Generate a fresh test keypair and write it where the operator named.
///
/// This is the only way this service ever obtains a key.  It refuses to
/// overwrite an existing file, so a `keygen` typo can never destroy a key, and
/// it refuses the same paths [`require_safe_keypair_path`] refuses, so it can
/// never create a file that then has to be refused on load.
pub fn generate_keypair_file(path: &Path, home: Option<&Path>) -> Result<[u8; ID_BYTES]> {
    require_safe_keypair_path(path, home)?;
    if path.exists() {
        return Err(RelayerError::UnsafeKeypairPath {
            path: path.to_path_buf(),
            reason: "a file already exists there; refusing to overwrite a key".to_owned(),
        });
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|source| RelayerError::io(parent, source))?;
    }

    let key = SigningKey::generate(&mut rand_core::OsRng);
    let public = key.verifying_key().to_bytes();
    let mut file_bytes = Vec::with_capacity(KEYPAIR_FILE_BYTES);
    file_bytes.extend_from_slice(&key.to_bytes());
    file_bytes.extend_from_slice(&public);
    let json = serde_json::to_string(&file_bytes)
        .map_err(|source| RelayerError::Serialization(source.to_string()))?;
    std::fs::write(path, json).map_err(|source| RelayerError::io(path, source))?;
    restrict_permissions(path)?;
    Ok(public)
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, permissions).map_err(|source| RelayerError::io(path, source))
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    // No portable equivalent.  The operator is told to restrict it by hand in
    // the `keygen` output rather than being told nothing.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/tester")
    }

    #[test]
    fn the_default_solana_wallet_store_is_refused_even_when_named_explicitly() {
        let path = home().join(".config/solana/id.json");
        let error = require_safe_keypair_path(&path, Some(&home())).unwrap_err();
        assert!(
            matches!(error, RelayerError::UnsafeKeypairPath { .. }),
            "expected a path refusal, got {error:?}"
        );
    }

    #[test]
    fn a_config_solana_path_outside_home_is_refused_lexically() {
        let path = PathBuf::from("/srv/keys/.config/solana/relayer.json");
        assert!(require_safe_keypair_path(&path, None).is_err());
    }

    #[test]
    fn an_ssh_path_is_refused() {
        assert!(
            require_safe_keypair_path(Path::new("/home/tester/.ssh/id_ed25519"), None).is_err()
        );
        assert!(require_safe_keypair_path(&home().join(".ssh/anything"), Some(&home())).is_err());
    }

    #[test]
    fn any_home_dotconfig_path_is_refused_even_without_solana_in_it() {
        let path = home().join(".config/somethingelse/key.json");
        assert!(require_safe_keypair_path(&path, Some(&home())).is_err());
    }

    #[test]
    fn an_ordinary_operator_named_path_is_admitted() {
        require_safe_keypair_path(
            Path::new("/srv/dclutch/keys/attestation.json"),
            Some(&home()),
        )
        .expect("an explicit non-wallet path is fine");
        require_safe_keypair_path(Path::new("./keys/attestation.json"), Some(&home()))
            .expect("a relative path under the working directory is fine");
    }

    #[test]
    fn tilde_expands_before_the_rules_are_applied() {
        let expanded = expand_tilde(Path::new("~/.config/solana/id.json"), Some(&home()));
        assert_eq!(expanded, home().join(".config/solana/id.json"));
        assert!(require_safe_keypair_path(&expanded, Some(&home())).is_err());
    }

    #[test]
    fn generate_then_load_round_trips_and_signs_verifiably() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested/attestation.json");
        let public = generate_keypair_file(&path, None).expect("generate");
        let signer = AttestationSigner::load(&path, None).expect("load");
        assert_eq!(signer.public_key(), public);

        let message = b"dclutch relayed attestation test vector";
        let signature = signer.sign(message);
        assert!(signer.verify(message, &signature));
        assert!(!verify_detached(&public, b"other bytes", &signature));
    }

    #[test]
    fn generate_refuses_to_overwrite_an_existing_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("attestation.json");
        generate_keypair_file(&path, None).expect("first generate");
        assert!(generate_keypair_file(&path, None).is_err());
    }

    #[test]
    fn a_keypair_file_whose_halves_disagree_refuses() {
        let mut raw = vec![7u8; 64];
        if let Some(byte) = raw.get_mut(63) {
            *byte = 9;
        }
        let error = AttestationSigner::from_file_bytes(&raw, Path::new("/tmp/x.json")).unwrap_err();
        assert!(matches!(error, RelayerError::MalformedKeypair { .. }));
    }

    #[test]
    fn a_keypair_file_of_the_wrong_width_refuses() {
        assert!(AttestationSigner::from_file_bytes(&[0u8; 32], Path::new("/tmp/x.json")).is_err());
        assert!(AttestationSigner::from_file_bytes(&[0u8; 65], Path::new("/tmp/x.json")).is_err());
    }
}
