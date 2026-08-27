//! The two derivations the daemon performs, and it performs no others.
//!
//! Both are "encode the canonical preimage in the wire crate, then SHA-256 it".
//! The wire crate computes no digests — it hands out the exact preimage and
//! compares the caller's result by equality — so the hash function lives here
//! and the layout does not.  Nothing in this file re-declares a byte offset, a
//! domain string, or a separator.

use dclutch_relay_contract::release::{
    AccountSetEntryV1, SET_DIGEST_SEED_PREIMAGE_BYTES, account_set_id_preimage_len_v1,
    encode_account_set_id_preimage_v1, encode_set_digest_seed_preimage_v1,
};
use sha2::{Digest, Sha256};

use crate::error::{RelayerError, Result};
use crate::id32::ID_BYTES;

/// Derive `account_set_id` from the founding-time ordered positions.
///
/// The operator never writes this value down: it is computed from the config's
/// ordered positions and printed so it can be pinned into the release. A
/// hand-written `account_set_id` would be a second authority for "which
/// accounts may be attested", and the whole point of the pin is that there is
/// exactly one.
pub fn derive_account_set_id(
    observed_cluster_id: [u8; ID_BYTES],
    relay_family_id: [u8; ID_BYTES],
    entries: &[AccountSetEntryV1],
) -> Result<[u8; ID_BYTES]> {
    let width = account_set_id_preimage_len_v1(entries.len())
        .map_err(|error| RelayerError::wire("account_set_id preimage width", error))?;
    let mut preimage = vec![0u8; width];
    encode_account_set_id_preimage_v1(&mut preimage, observed_cluster_id, relay_family_id, entries)
        .map_err(|error| RelayerError::wire("account_set_id preimage", error))?;
    Ok(Sha256::digest(&preimage).into())
}

/// The running fold that produces `set_digest`.
///
/// `running_0 = SHA-256(seed preimage)`, then `running_{i+1} = SHA-256(running_i
/// || body_i)` over the exact encoded observation bodies in set order.  Folding
/// rather than hashing the whole record is what keeps the on-chain check to one
/// 32-byte-plus-body hash per append.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetDigestFold {
    running: [u8; ID_BYTES],
    absorbed: u16,
}

impl SetDigestFold {
    /// Seed the fold for one `(account_set_id, observed_slot)` pair.
    pub fn seed(account_set_id: [u8; ID_BYTES], observed_slot: u64) -> Result<Self> {
        let mut preimage = [0u8; SET_DIGEST_SEED_PREIMAGE_BYTES];
        encode_set_digest_seed_preimage_v1(&mut preimage, account_set_id, observed_slot)
            .map_err(|error| RelayerError::wire("set digest seed preimage", error))?;
        Ok(Self {
            running: Sha256::digest(preimage).into(),
            absorbed: 0,
        })
    }

    /// Absorb one exact encoded observation body.
    pub fn absorb(&mut self, body: &[u8]) {
        let mut hasher = Sha256::new();
        hasher.update(self.running);
        hasher.update(body);
        self.running = hasher.finalize().into();
        self.absorbed = self.absorbed.saturating_add(1);
    }

    /// The digest after everything absorbed so far.
    pub const fn digest(&self) -> [u8; ID_BYTES] {
        self.running
    }

    /// How many bodies have been absorbed.
    pub const fn absorbed(&self) -> u16 {
        self.absorbed
    }
}

/// SHA-256 over an arbitrary byte string, used for artifact self-description.
pub fn sha256(bytes: &[u8]) -> [u8; ID_BYTES] {
    Sha256::digest(bytes).into()
}

/// An incremental SHA-256 over an account's paged tail.
///
/// A 2.3 MB `ProgramData` body never exists in memory in one piece: pages are
/// absorbed as they arrive and dropped.
#[derive(Default)]
pub struct TailHasher {
    hasher: Sha256,
    absorbed: u64,
}

impl TailHasher {
    /// Start an empty tail hash.
    pub fn new() -> Self {
        Self::default()
    }

    /// Absorb one page of tail bytes.
    pub fn absorb(&mut self, page: &[u8]) {
        self.hasher.update(page);
        self.absorbed = self
            .absorbed
            .saturating_add(u64::try_from(page.len()).unwrap_or(u64::MAX));
    }

    /// How many tail bytes have been absorbed.
    pub const fn absorbed(&self) -> u64 {
        self.absorbed
    }

    /// Finish the hash.
    pub fn finish(self) -> [u8; ID_BYTES] {
        self.hasher.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_relay_contract::SHA256_EMPTY_DIGEST;

    fn entry(key: u8, owner: u8, inline_len: u16) -> AccountSetEntryV1 {
        AccountSetEntryV1 {
            key: [key; ID_BYTES],
            expected_owner: [owner; ID_BYTES],
            inline_len,
        }
    }

    /// Spell the §4.3 preimage out by hand, independently of the wire crate's
    /// encoder, and require byte equality.  Calling the encoder twice would
    /// prove nothing; this is the check that the daemon and the document agree.
    #[test]
    fn the_account_set_id_preimage_is_the_one_the_design_document_spells() {
        let observed_cluster_id = dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1;
        let relay_family_id = dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1;
        let entries = [entry(1, 2, 36), entry(3, 4, 45), entry(5, 6, 416)];

        let mut hand = Vec::new();
        hand.extend_from_slice(b"dclutch/relayed-account-set/v1");
        hand.push(0);
        hand.extend_from_slice(&observed_cluster_id);
        hand.push(0);
        hand.extend_from_slice(&relay_family_id);
        hand.push(0);
        hand.extend_from_slice(&3u16.to_le_bytes());
        hand.push(0);
        for item in &entries {
            hand.extend_from_slice(&item.key);
            hand.extend_from_slice(&item.expected_owner);
            hand.extend_from_slice(&item.inline_len.to_le_bytes());
        }
        assert_eq!(hand.len(), 100 + 3 * 66);

        let width = account_set_id_preimage_len_v1(entries.len()).expect("width");
        let mut encoded = vec![0u8; width];
        encode_account_set_id_preimage_v1(
            &mut encoded,
            observed_cluster_id,
            relay_family_id,
            &entries,
        )
        .expect("encode");
        assert_eq!(encoded, hand, "the encoder and the document disagree");

        let derived =
            derive_account_set_id(observed_cluster_id, relay_family_id, &entries).expect("derive");
        assert_eq!(derived, sha256(&hand));
    }

    #[test]
    fn reordering_the_positions_changes_the_account_set_id() {
        let cluster = dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1;
        let family = dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1;
        let forward = [entry(1, 2, 36), entry(3, 4, 45)];
        let backward = [entry(3, 4, 45), entry(1, 2, 36)];
        assert_ne!(
            derive_account_set_id(cluster, family, &forward).expect("a"),
            derive_account_set_id(cluster, family, &backward).expect("b")
        );
    }

    #[test]
    fn changing_only_an_inline_width_changes_the_account_set_id() {
        let cluster = dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1;
        let family = dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1;
        assert_ne!(
            derive_account_set_id(cluster, family, &[entry(1, 2, 36)]).expect("a"),
            derive_account_set_id(cluster, family, &[entry(1, 2, 37)]).expect("b")
        );
    }

    #[test]
    fn the_same_cluster_on_a_different_cluster_id_derives_a_different_set() {
        let family = dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1;
        let entries = [entry(1, 2, 36)];
        assert_ne!(
            derive_account_set_id(
                dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1,
                family,
                &entries
            )
            .expect("mainnet"),
            derive_account_set_id(
                dclutch_relay_contract::SOLANA_DEVNET_GENESIS_HASH_V1,
                family,
                &entries
            )
            .expect("devnet")
        );
    }

    #[test]
    fn a_set_wider_than_the_release_ceiling_refuses_rather_than_truncating() {
        let cluster = dclutch_relay_contract::SOLANA_MAINNET_GENESIS_HASH_V1;
        let family = dclutch_relay_contract::RELAYED_FAMILY_RELEASE_ID_V1;
        let too_many: Vec<AccountSetEntryV1> = (1..=9).map(|i| entry(i, 2, 8)).collect();
        assert!(derive_account_set_id(cluster, family, &too_many).is_err());
        assert!(derive_account_set_id(cluster, family, &[]).is_err());
    }

    /// The fold, spelled by hand against §4.3's two lines.
    #[test]
    fn the_set_digest_fold_matches_the_documents_recurrence() {
        let account_set_id = [0x5au8; ID_BYTES];
        let observed_slot = 423_941_138u64;
        let bodies: [&[u8]; 3] = [b"body-zero", b"body-one", b"body-two"];

        let mut seed = Vec::new();
        seed.extend_from_slice(b"dclutch/relayed-set/v1");
        seed.push(0);
        seed.extend_from_slice(&account_set_id);
        seed.extend_from_slice(&observed_slot.to_le_bytes());
        assert_eq!(seed.len(), SET_DIGEST_SEED_PREIMAGE_BYTES);

        let mut expected = sha256(&seed);
        for body in bodies {
            let mut next = Vec::new();
            next.extend_from_slice(&expected);
            next.extend_from_slice(body);
            expected = sha256(&next);
        }

        let mut fold = SetDigestFold::seed(account_set_id, observed_slot).expect("seed");
        for body in bodies {
            fold.absorb(body);
        }
        assert_eq!(fold.digest(), expected);
        assert_eq!(fold.absorbed(), 3);
    }

    #[test]
    fn the_fold_is_slot_bound_and_order_sensitive() {
        let id = [0x5au8; ID_BYTES];
        let mut at_one = SetDigestFold::seed(id, 1).expect("seed");
        let mut at_two = SetDigestFold::seed(id, 2).expect("seed");
        at_one.absorb(b"same");
        at_two.absorb(b"same");
        assert_ne!(at_one.digest(), at_two.digest());

        let mut forward = SetDigestFold::seed(id, 1).expect("seed");
        forward.absorb(b"a");
        forward.absorb(b"b");
        let mut backward = SetDigestFold::seed(id, 1).expect("seed");
        backward.absorb(b"b");
        backward.absorb(b"a");
        assert_ne!(forward.digest(), backward.digest());
    }

    #[test]
    fn an_empty_tail_hashes_to_the_pinned_empty_string_digest() {
        // A fully inline body's `tail_digest` is not a special case in this
        // daemon: it falls out of hashing a zero-length tail, and the constant
        // the wire crate pins is what it must equal.
        assert_eq!(TailHasher::new().finish(), SHA256_EMPTY_DIGEST);
    }
}
