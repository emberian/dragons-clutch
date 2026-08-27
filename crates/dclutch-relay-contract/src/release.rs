//! Immutable release records: the relayer key set, the adapter configuration,
//! and the two canonical digest preimages this family derives its identities
//! from.
//!
//! Nothing here hashes.  Each preimage builder writes exactly the bytes a
//! caller must hash, so the SBF adapter uses the runtime's SHA-256 syscall, the
//! daemon uses an ordinary software implementation, and both agree by
//! construction rather than by two independent transcriptions of a rule.

use crate::{
    ADDRESS_BYTES, Error, MAX_RELAYED_ACCOUNTS_V1, MAX_RELAYED_INLINE_BYTES_V1,
    MAX_RELAYER_KEYS_V1, RELAYED_ACCOUNT_SET_DOMAIN_V1,
    RELAYED_ADAPTER_CONFIG_ACCOUNT_SET_ID_OFFSET, RELAYED_ADAPTER_CONFIG_BYTES,
    RELAYED_ADAPTER_CONFIG_MAGIC, RELAYED_ADAPTER_CONFIG_MAX_CLUSTER_SKEW_SECONDS_OFFSET,
    RELAYED_ADAPTER_CONFIG_MAX_OBSERVATION_AGE_SECONDS_OFFSET,
    RELAYED_ADAPTER_CONFIG_OBSERVABLE_SELECTOR_OFFSET, RELAYED_ADAPTER_CONFIG_RAW_EXPONENT_OFFSET,
    RELAYED_ADAPTER_CONFIG_RESERVED_OFFSET, RELAYED_ADAPTER_CONFIG_RESERVED_TAIL_OFFSET,
    RELAYED_SET_DIGEST_DOMAIN_V1, RELAYER_KEY_SET_BYTES, RELAYER_KEY_SET_KEY_COUNT_OFFSET,
    RELAYER_KEY_SET_KEYS_OFFSET, RELAYER_KEY_SET_MAGIC, RELAYER_KEY_SET_RESERVED_OFFSET,
    RELAYER_KEY_SET_SEAL_THRESHOLD_OFFSET, Result, array, base, header, i32_at, is_zero, one, put,
    require_nonzero, require_zero, u16_from, u32_at, u64_at,
};

/// One position of the founding-time pinned ordered account set.
///
/// The relayer chooses none of these: it echoes the identity and the adapter
/// compares.  Because `inline_len` is pinned per position, a relayer that
/// inlines a different prefix produces a different `account_set_id` and its
/// attestation is refused before any byte of `data` is read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountSetEntryV1 {
    /// The exact observed account address.
    pub key: [u8; ADDRESS_BYTES],
    /// The exact owning program the account must report.
    pub expected_owner: [u8; ADDRESS_BYTES],
    /// The exact inline prefix width the relayer must carry.
    pub inline_len: u16,
}

/// Bytes contributed to the `account_set_id` preimage by one position.
pub const ACCOUNT_SET_ENTRY_PREIMAGE_BYTES: usize = 66;
/// Fixed bytes of the `account_set_id` preimage before the entries.
pub const ACCOUNT_SET_PREIMAGE_HEADER_BYTES: usize = 100;
/// Exact width of the running set-digest seed preimage.
pub const SET_DIGEST_SEED_PREIMAGE_BYTES: usize = 63;

/// Exact canonical preimage width for an ordered set of `entries.len()`.
pub fn account_set_id_preimage_len_v1(entry_count: usize) -> Result<usize> {
    if entry_count == 0 || entry_count > MAX_RELAYED_ACCOUNTS_V1 {
        return Err(Error::InvalidSetGeometry);
    }
    ACCOUNT_SET_PREIMAGE_HEADER_BYTES
        .checked_add(
            entry_count
                .checked_mul(ACCOUNT_SET_ENTRY_PREIMAGE_BYTES)
                .ok_or(Error::ArithmeticOverflow)?,
        )
        .ok_or(Error::ArithmeticOverflow)
}

/// Decode one wire-carried account-set entry.
///
/// The wire form of an entry *is* its contribution to the `account_set_id`
/// preimage, byte for byte, so a caller that re-encodes what it decoded here
/// cannot produce a different identity by transcribing a field twice.
pub fn decode_account_set_entry_v1(entries: &[u8], index: usize) -> Result<AccountSetEntryV1> {
    let offset = index
        .checked_mul(ACCOUNT_SET_ENTRY_PREIMAGE_BYTES)
        .ok_or(Error::ArithmeticOverflow)?;
    let key: [u8; ADDRESS_BYTES] = crate::array(entries, offset)?;
    let owner_offset = offset.checked_add(32).ok_or(Error::ArithmeticOverflow)?;
    let expected_owner: [u8; ADDRESS_BYTES] = crate::array(entries, owner_offset)?;
    let width_offset = offset.checked_add(64).ok_or(Error::ArithmeticOverflow)?;
    let inline_len = crate::u16_at(entries, width_offset)?;
    require_nonzero(&key)?;
    require_nonzero(&expected_owner)?;
    if usize::from(inline_len) > MAX_RELAYED_INLINE_BYTES_V1 {
        return Err(Error::InvalidInlineWidth);
    }
    Ok(AccountSetEntryV1 {
        key,
        expected_owner,
        inline_len,
    })
}

/// Encode the one canonical `account_set_id` preimage.
///
/// The caller hashes exactly the written bytes.  Every field separator is an
/// explicit zero byte so no two distinct sets can share a preimage by running
/// one field's tail into the next.
pub fn encode_account_set_id_preimage_v1(
    output: &mut [u8],
    observed_cluster_id: [u8; 32],
    relay_family_id: [u8; 32],
    entries: &[AccountSetEntryV1],
) -> Result<usize> {
    let width = account_set_id_preimage_len_v1(entries.len())?;
    if output.len() != width {
        return Err(Error::OutputLength);
    }
    require_nonzero(&observed_cluster_id)?;
    require_nonzero(&relay_family_id)?;
    for entry in entries {
        require_nonzero(&entry.key)?;
        require_nonzero(&entry.expected_owner)?;
        if usize::from(entry.inline_len) > MAX_RELAYED_INLINE_BYTES_V1 {
            return Err(Error::InvalidInlineWidth);
        }
    }

    let mut cursor = 0usize;
    let mut write = |bytes: &[u8], cursor: &mut usize| -> Result<()> {
        put(output, *cursor, bytes)?;
        *cursor = cursor.checked_add(bytes.len()).ok_or(Error::OutputLength)?;
        Ok(())
    };
    write(RELAYED_ACCOUNT_SET_DOMAIN_V1, &mut cursor)?;
    write(&[0], &mut cursor)?;
    write(&observed_cluster_id, &mut cursor)?;
    write(&[0], &mut cursor)?;
    write(&relay_family_id, &mut cursor)?;
    write(&[0], &mut cursor)?;
    write(&u16_from(entries.len())?.to_le_bytes(), &mut cursor)?;
    write(&[0], &mut cursor)?;
    for entry in entries {
        write(&entry.key, &mut cursor)?;
        write(&entry.expected_owner, &mut cursor)?;
        write(&entry.inline_len.to_le_bytes(), &mut cursor)?;
    }
    if cursor != width {
        return Err(Error::OutputLength);
    }
    Ok(width)
}

/// Encode the seed preimage of the running set-digest fold.
///
/// `running_0 = SHA-256(domain || 0x00 || account_set_id || observed_slot LE)`,
/// and every later step is `SHA-256(running_i || body_i)` over the accepted
/// body bytes, so the fold order is canonical and no large on-chain hash of the
/// whole record is ever needed.
pub fn encode_set_digest_seed_preimage_v1(
    output: &mut [u8],
    account_set_id: [u8; 32],
    observed_slot: u64,
) -> Result<usize> {
    if output.len() != SET_DIGEST_SEED_PREIMAGE_BYTES {
        return Err(Error::OutputLength);
    }
    require_nonzero(&account_set_id)?;
    let mut cursor = 0usize;
    put(output, cursor, RELAYED_SET_DIGEST_DOMAIN_V1)?;
    cursor = cursor
        .checked_add(RELAYED_SET_DIGEST_DOMAIN_V1.len())
        .ok_or(Error::OutputLength)?;
    put(output, cursor, &[0])?;
    cursor = cursor.checked_add(1).ok_or(Error::OutputLength)?;
    put(output, cursor, &account_set_id)?;
    cursor = cursor.checked_add(32).ok_or(Error::OutputLength)?;
    put(output, cursor, &observed_slot.to_le_bytes())?;
    cursor = cursor.checked_add(8).ok_or(Error::OutputLength)?;
    if cursor != SET_DIGEST_SEED_PREIMAGE_BYTES {
        return Err(Error::OutputLength);
    }
    Ok(SET_DIGEST_SEED_PREIMAGE_BYTES)
}

/// The immutable relayer key set.
///
/// Its content identity *is* `ProviderReleaseV1.provider_deployment_release_id`,
/// which is the whole design: rotation means a new key set, hence a new provider
/// release, hence a new Source spec, hence a new Source material, hence a new
/// Market generation.  The key set cannot change under the holders.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayerKeySetV1 {
    keys: [[u8; ADDRESS_BYTES]; MAX_RELAYER_KEYS_V1],
    key_count: u8,
    seal_threshold: u8,
}

impl RelayerKeySetV1 {
    /// Construct one canonical key set from strictly ascending key material.
    pub fn new(keys: &[[u8; ADDRESS_BYTES]], seal_threshold: u8) -> Result<Self> {
        if keys.is_empty() || keys.len() > MAX_RELAYER_KEYS_V1 {
            return Err(Error::NonCanonicalKeySet);
        }
        let key_count = u8::try_from(keys.len()).map_err(|_| Error::NonCanonicalKeySet)?;
        if seal_threshold == 0 || seal_threshold > key_count {
            return Err(Error::NonCanonicalKeySet);
        }
        let mut slots = [[0u8; ADDRESS_BYTES]; MAX_RELAYER_KEYS_V1];
        let mut previous: Option<[u8; ADDRESS_BYTES]> = None;
        for (index, key) in keys.iter().enumerate() {
            if is_zero(key) {
                return Err(Error::NonCanonicalKeySet);
            }
            if previous.is_some_and(|earlier| earlier.as_slice() >= key.as_slice()) {
                return Err(Error::NonCanonicalKeySet);
            }
            previous = Some(*key);
            *slots.get_mut(index).ok_or(Error::NonCanonicalKeySet)? = *key;
        }
        Ok(Self {
            keys: slots,
            key_count,
            seal_threshold,
        })
    }

    /// Hostile-decode one exactly 176-byte key-set record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(bytes, RELAYER_KEY_SET_BYTES, RELAYER_KEY_SET_MAGIC)?;
        require_zero(bytes, RELAYER_KEY_SET_RESERVED_OFFSET, 4)?;
        let key_count = one(bytes, RELAYER_KEY_SET_KEY_COUNT_OFFSET)?;
        let seal_threshold = one(bytes, RELAYER_KEY_SET_SEAL_THRESHOLD_OFFSET)?;
        let declared = usize::from(key_count);
        if declared == 0 || declared > MAX_RELAYER_KEYS_V1 {
            return Err(Error::NonCanonicalKeySet);
        }
        let mut material = [[0u8; ADDRESS_BYTES]; MAX_RELAYER_KEYS_V1];
        for index in 0..MAX_RELAYER_KEYS_V1 {
            let offset = RELAYER_KEY_SET_KEYS_OFFSET
                .checked_add(index.checked_mul(32).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            let key: [u8; ADDRESS_BYTES] = array(bytes, offset)?;
            if index >= declared {
                // An unused slot must be zero: trailing key material would be a
                // second, invisible member of a set the release pins by digest.
                if !is_zero(&key) {
                    return Err(Error::NonCanonicalKeySet);
                }
            } else {
                *material.get_mut(index).ok_or(Error::NonCanonicalKeySet)? = key;
            }
        }
        let used = material.get(..declared).ok_or(Error::NonCanonicalKeySet)?;
        Self::new(used, seal_threshold)
    }

    /// Encode the exact canonical key-set bytes.
    pub fn to_bytes(self) -> Result<[u8; RELAYER_KEY_SET_BYTES]> {
        let mut out = base::<RELAYER_KEY_SET_BYTES>(RELAYER_KEY_SET_MAGIC)?;
        put(
            &mut out,
            RELAYER_KEY_SET_KEY_COUNT_OFFSET,
            &[self.key_count],
        )?;
        put(
            &mut out,
            RELAYER_KEY_SET_SEAL_THRESHOLD_OFFSET,
            &[self.seal_threshold],
        )?;
        for (index, key) in self.keys().iter().enumerate() {
            let offset = RELAYER_KEY_SET_KEYS_OFFSET
                .checked_add(index.checked_mul(32).ok_or(Error::ArithmeticOverflow)?)
                .ok_or(Error::ArithmeticOverflow)?;
            put(&mut out, offset, key)?;
        }
        Ok(out)
    }

    /// The ordered member keys.
    pub fn keys(&self) -> &[[u8; ADDRESS_BYTES]] {
        match self.keys.get(..usize::from(self.key_count)) {
            Some(members) => members,
            None => &[],
        }
    }

    /// The number of keys in the set.
    pub const fn key_count(self) -> u8 {
        self.key_count
    }

    /// The number of distinct members required to seal a record.
    pub const fn seal_threshold(self) -> u8 {
        self.seal_threshold
    }

    /// Position of one key in the set, or [`Error::UnknownSigner`].
    ///
    /// Membership is the *only* thing that authorizes a signature in this
    /// family.  Instruction adjacency selects which precompile instruction to
    /// parse; the release-pinned key and byte-exact message equality are the
    /// authority.
    pub fn require_member(&self, key: &[u8; ADDRESS_BYTES]) -> Result<u8> {
        for (index, member) in self.keys().iter().enumerate() {
            if member == key {
                return u8::try_from(index).map_err(|_| Error::NonCanonicalKeySet);
            }
        }
        Err(Error::UnknownSigner)
    }
}

/// The founding-time adapter pin for one relayed source.
///
/// This is the analogue of `PythAdapterConfigV1`, widened from the design
/// document's 64 bytes to 80 so it can carry the house 16-byte magic and schema
/// header.  Without that header a 64-byte raw record of another family decodes
/// in the same slot, and content-ID binding alone does not catch a founder who
/// pinned the wrong record kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelayedAdapterConfigV1 {
    account_set_id: [u8; 32],
    observable_selector: u32,
    raw_exponent: i32,
    max_observation_age_seconds: u64,
    max_cluster_skew_seconds: u64,
}

impl RelayedAdapterConfigV1 {
    /// Construct one adapter configuration.
    pub fn new(
        account_set_id: [u8; 32],
        observable_selector: u32,
        raw_exponent: i32,
        max_observation_age_seconds: u64,
        max_cluster_skew_seconds: u64,
    ) -> Result<Self> {
        require_nonzero(&account_set_id)?;
        if max_observation_age_seconds == 0
            || max_cluster_skew_seconds >= max_observation_age_seconds
        {
            // A skew allowance at or above the staleness bound would make the
            // bound unenforceable: every attestation would be admissible by
            // claiming the clocks disagree.
            return Err(Error::ClusterSkewExceedsWindowGrace);
        }
        Ok(Self {
            account_set_id,
            observable_selector,
            raw_exponent,
            max_observation_age_seconds,
            max_cluster_skew_seconds,
        })
    }

    /// Hostile-decode one exactly 80-byte adapter configuration.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        header(
            bytes,
            RELAYED_ADAPTER_CONFIG_BYTES,
            RELAYED_ADAPTER_CONFIG_MAGIC,
        )?;
        require_zero(bytes, RELAYED_ADAPTER_CONFIG_RESERVED_OFFSET, 6)?;
        require_zero(bytes, RELAYED_ADAPTER_CONFIG_RESERVED_TAIL_OFFSET, 8)?;
        Self::new(
            array(bytes, RELAYED_ADAPTER_CONFIG_ACCOUNT_SET_ID_OFFSET)?,
            u32_at(bytes, RELAYED_ADAPTER_CONFIG_OBSERVABLE_SELECTOR_OFFSET)?,
            i32_at(bytes, RELAYED_ADAPTER_CONFIG_RAW_EXPONENT_OFFSET)?,
            u64_at(
                bytes,
                RELAYED_ADAPTER_CONFIG_MAX_OBSERVATION_AGE_SECONDS_OFFSET,
            )?,
            u64_at(
                bytes,
                RELAYED_ADAPTER_CONFIG_MAX_CLUSTER_SKEW_SECONDS_OFFSET,
            )?,
        )
    }

    /// Encode the exact canonical adapter-configuration bytes.
    pub fn to_bytes(self) -> Result<[u8; RELAYED_ADAPTER_CONFIG_BYTES]> {
        let mut out = base::<RELAYED_ADAPTER_CONFIG_BYTES>(RELAYED_ADAPTER_CONFIG_MAGIC)?;
        put(
            &mut out,
            RELAYED_ADAPTER_CONFIG_ACCOUNT_SET_ID_OFFSET,
            &self.account_set_id,
        )?;
        put(
            &mut out,
            RELAYED_ADAPTER_CONFIG_OBSERVABLE_SELECTOR_OFFSET,
            &self.observable_selector.to_le_bytes(),
        )?;
        put(
            &mut out,
            RELAYED_ADAPTER_CONFIG_RAW_EXPONENT_OFFSET,
            &self.raw_exponent.to_le_bytes(),
        )?;
        put(
            &mut out,
            RELAYED_ADAPTER_CONFIG_MAX_OBSERVATION_AGE_SECONDS_OFFSET,
            &self.max_observation_age_seconds.to_le_bytes(),
        )?;
        put(
            &mut out,
            RELAYED_ADAPTER_CONFIG_MAX_CLUSTER_SKEW_SECONDS_OFFSET,
            &self.max_cluster_skew_seconds.to_le_bytes(),
        )?;
        Ok(out)
    }

    /// The founding-time pinned ordered account set.
    pub const fn account_set_id(self) -> [u8; 32] {
        self.account_set_id
    }
    /// Which observable of the decoding-rules table this source produces.
    pub const fn observable_selector(self) -> u32 {
        self.observable_selector
    }
    /// The declared base-ten scale of the produced atom.
    pub const fn raw_exponent(self) -> i32 {
        self.raw_exponent
    }
    /// The staleness bound spanning the two clusters' clocks.
    pub const fn max_observation_age_seconds(self) -> u64 {
        self.max_observation_age_seconds
    }
    /// The explicitly named, separately checkable two-clock skew allowance.
    pub const fn max_cluster_skew_seconds(self) -> u64 {
        self.max_cluster_skew_seconds
    }

    /// Founding-time admission: the window's own liveness grace must cover the
    /// declared skew allowance, so skew alone can never trigger the funded
    /// permissionless failure walk.
    pub fn require_window_admits_skew(self, window_max_age_seconds: u32) -> Result<()> {
        if u64::from(window_max_age_seconds) < self.max_cluster_skew_seconds {
            return Err(Error::ClusterSkewExceedsWindowGrace);
        }
        Ok(())
    }

    /// The two-clock staleness join.
    ///
    /// `current_unix_seconds` is the **devnet** `Clock` the SBF adapter supplies;
    /// `observed_unix_seconds` is decoded from the **attested mainnet** `Clock`
    /// sysvar account.  A withheld-and-replayed attestation is caught here and
    /// nowhere else: the relayer cannot forge mainnet time, but it can hold a
    /// signed message, so this bound must be tight and is a bound rather than an
    /// assumption of relayer promptness.
    pub fn require_observation_freshness(
        self,
        current_unix_seconds: i64,
        observed_unix_seconds: i64,
    ) -> Result<()> {
        let age = current_unix_seconds
            .checked_sub(observed_unix_seconds)
            .ok_or(Error::ArithmeticOverflow)?;
        let max_age = i64::try_from(self.max_observation_age_seconds)
            .map_err(|_| Error::ArithmeticOverflow)?;
        if age > max_age {
            return Err(Error::ObservationTooStale);
        }
        let skew =
            i64::try_from(self.max_cluster_skew_seconds).map_err(|_| Error::ArithmeticOverflow)?;
        let negated = skew.checked_neg().ok_or(Error::ArithmeticOverflow)?;
        if age < negated {
            return Err(Error::ObservationFromTheFuture);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        RELAYED_ADAPTER_CONFIG_BYTES, RELAYED_ADAPTER_CONFIG_EXAMPLE,
        RELAYED_ADAPTER_CONFIG_REFUSAL_CORPUS,
        RELAYED_ADAPTER_CONFIG_REFUSAL_CORPUS_CANONICAL_WIDTH,
        RELAYED_ADAPTER_CONFIG_REFUSAL_CORPUS_COUNT, RELAYER_KEY_SET_EXAMPLE,
        RELAYER_KEY_SET_REFUSAL_CORPUS, RELAYER_KEY_SET_REFUSAL_CORPUS_CANONICAL_WIDTH,
        RELAYER_KEY_SET_REFUSAL_CORPUS_COUNT, RELAYER_KEY_SET_SINGLETON_EXAMPLE,
    };

    const ASCENDING: [[u8; 32]; 3] = [[1; 32], [2; 32], [3; 32]];

    #[test]
    fn the_generated_corpus_widths_match_the_rust_record_widths() {
        assert_eq!(
            RELAYER_KEY_SET_REFUSAL_CORPUS_CANONICAL_WIDTH,
            crate::RELAYER_KEY_SET_BYTES
        );
        assert_eq!(
            RELAYED_ADAPTER_CONFIG_REFUSAL_CORPUS_CANONICAL_WIDTH,
            RELAYED_ADAPTER_CONFIG_BYTES
        );
    }

    #[test]
    fn the_lean_key_set_examples_decode_and_reencode() {
        for example in [&RELAYER_KEY_SET_EXAMPLE, &RELAYER_KEY_SET_SINGLETON_EXAMPLE] {
            let decoded = RelayerKeySetV1::decode(example).expect("key set decodes");
            assert_eq!(&decoded.to_bytes().expect("encode"), example);
        }
    }

    #[test]
    fn the_lean_key_set_refusal_corpus_is_refused() {
        assert_eq!(
            RELAYER_KEY_SET_REFUSAL_CORPUS.len(),
            RELAYER_KEY_SET_REFUSAL_CORPUS_COUNT,
            "the Rust side is iterating fewer entries than Lean emitted"
        );
        for (index, candidate) in RELAYER_KEY_SET_REFUSAL_CORPUS.iter().enumerate() {
            assert!(
                RelayerKeySetV1::decode(candidate).is_err(),
                "key-set corpus entry {index} was accepted"
            );
        }
    }

    #[test]
    fn duplicate_and_descending_members_are_structurally_impossible() {
        assert_eq!(
            RelayerKeySetV1::new(&[[2; 32], [2; 32]], 1),
            Err(Error::NonCanonicalKeySet)
        );
        assert_eq!(
            RelayerKeySetV1::new(&[[3; 32], [1; 32]], 1),
            Err(Error::NonCanonicalKeySet)
        );
    }

    #[test]
    fn a_threshold_above_the_cardinality_refuses() {
        assert_eq!(
            RelayerKeySetV1::new(&[[1; 32], [2; 32]], 3),
            Err(Error::NonCanonicalKeySet)
        );
        assert_eq!(
            RelayerKeySetV1::new(&[[1; 32]], 0),
            Err(Error::NonCanonicalKeySet)
        );
    }

    #[test]
    fn key_material_in_an_unused_slot_refuses() {
        let set = RelayerKeySetV1::new(&[[1; 32], [2; 32]], 1).expect("set");
        let mut bytes = set.to_bytes().expect("encode");
        let ghost = RELAYER_KEY_SET_KEYS_OFFSET + 4 * 32;
        put(&mut bytes, ghost, &[0xab]).expect("write");
        assert_eq!(
            RelayerKeySetV1::decode(&bytes),
            Err(Error::NonCanonicalKeySet)
        );
    }

    #[test]
    fn a_signer_outside_the_set_is_refused_by_membership_not_by_adjacency() {
        let set = RelayerKeySetV1::new(&ASCENDING, 2).expect("set");
        assert_eq!(set.require_member(&[1; 32]), Ok(0));
        assert_eq!(set.require_member(&[3; 32]), Ok(2));
        assert_eq!(set.require_member(&[4; 32]), Err(Error::UnknownSigner));
    }

    #[test]
    fn the_lean_adapter_config_example_round_trips_and_its_corpus_refuses() {
        let decoded =
            RelayedAdapterConfigV1::decode(&RELAYED_ADAPTER_CONFIG_EXAMPLE).expect("config");
        assert_eq!(
            decoded.to_bytes().expect("encode"),
            RELAYED_ADAPTER_CONFIG_EXAMPLE
        );
        assert_eq!(decoded.raw_exponent(), -8, "a signed scale lost its sign");
        assert_eq!(
            RELAYED_ADAPTER_CONFIG_REFUSAL_CORPUS.len(),
            RELAYED_ADAPTER_CONFIG_REFUSAL_CORPUS_COUNT
        );
        for (index, candidate) in RELAYED_ADAPTER_CONFIG_REFUSAL_CORPUS.iter().enumerate() {
            assert!(
                RelayedAdapterConfigV1::decode(candidate).is_err(),
                "adapter-config corpus entry {index} was accepted"
            );
        }
    }

    #[test]
    fn the_staleness_join_bounds_both_directions() {
        let config = RelayedAdapterConfigV1::new([6; 32], 0, -8, 5_400, 120).expect("config");
        assert_eq!(
            config.require_observation_freshness(1_000_000, 1_000_000),
            Ok(())
        );
        assert_eq!(
            config.require_observation_freshness(1_005_400, 1_000_000),
            Ok(())
        );
        assert_eq!(
            config.require_observation_freshness(1_005_401, 1_000_000),
            Err(Error::ObservationTooStale)
        );
        // Mainnet's clock running ahead of devnet's by more than the named
        // allowance is a distinct refusal from staleness, on purpose.
        assert_eq!(
            config.require_observation_freshness(1_000_000, 1_000_120),
            Ok(())
        );
        assert_eq!(
            config.require_observation_freshness(1_000_000, 1_000_121),
            Err(Error::ObservationFromTheFuture)
        );
    }

    #[test]
    fn skew_alone_can_never_expire_a_window_that_admits_it() {
        let config = RelayedAdapterConfigV1::new([6; 32], 0, -8, 5_400, 120).expect("config");
        assert_eq!(config.require_window_admits_skew(5_400), Ok(()));
        assert_eq!(config.require_window_admits_skew(120), Ok(()));
        assert_eq!(
            config.require_window_admits_skew(119),
            Err(Error::ClusterSkewExceedsWindowGrace)
        );
    }

    #[test]
    fn a_skew_allowance_that_swallows_the_staleness_bound_refuses_at_construction() {
        assert_eq!(
            RelayedAdapterConfigV1::new([6; 32], 0, 0, 600, 600),
            Err(Error::ClusterSkewExceedsWindowGrace)
        );
    }

    #[test]
    fn the_account_set_preimage_is_exact_and_separated() {
        let entries = [
            AccountSetEntryV1 {
                key: [1; 32],
                expected_owner: [2; 32],
                inline_len: 36,
            },
            AccountSetEntryV1 {
                key: [3; 32],
                expected_owner: [2; 32],
                inline_len: 45,
            },
        ];
        let width = account_set_id_preimage_len_v1(entries.len()).expect("width");
        assert_eq!(width, 100 + 2 * 66);
        let mut buffer = [0u8; 232];
        let written = encode_account_set_id_preimage_v1(&mut buffer, [9; 32], [8; 32], &entries)
            .expect("encode");
        assert_eq!(written, width);
        assert_eq!(buffer.get(..30), Some(RELAYED_ACCOUNT_SET_DOMAIN_V1));
        assert_eq!(buffer.get(30), Some(&0));

        // A different pinned inline width is a different set, which is what
        // makes "the relayer chose a different prefix" refuse before decoding.
        let mut widened = entries;
        widened.get_mut(1).expect("entry").inline_len = 46;
        let mut other = [0u8; 232];
        encode_account_set_id_preimage_v1(&mut other, [9; 32], [8; 32], &widened).expect("encode");
        assert_ne!(buffer, other);
    }

    #[test]
    fn a_wire_entry_decodes_to_the_bytes_the_preimage_would_have_written() {
        let entries = [
            AccountSetEntryV1 {
                key: [1; 32],
                expected_owner: [2; 32],
                inline_len: 36,
            },
            AccountSetEntryV1 {
                key: [3; 32],
                expected_owner: [4; 32],
                inline_len: 424,
            },
        ];
        let width = account_set_id_preimage_len_v1(entries.len()).expect("width");
        let mut preimage = [0u8; 232];
        encode_account_set_id_preimage_v1(&mut preimage, [9; 32], [8; 32], &entries)
            .expect("encode");
        let tail = preimage
            .get(ACCOUNT_SET_PREIMAGE_HEADER_BYTES..width)
            .expect("entry tail");
        for (index, expected) in entries.iter().enumerate() {
            assert_eq!(decode_account_set_entry_v1(tail, index), Ok(*expected));
        }
        assert_eq!(
            decode_account_set_entry_v1(tail, entries.len()),
            Err(Error::InvalidLength),
            "a read past the declared tail must refuse, not wrap"
        );
    }

    #[test]
    fn a_wire_entry_with_a_zero_key_or_owner_refuses() {
        let blank = [0u8; ACCOUNT_SET_ENTRY_PREIMAGE_BYTES];
        assert_eq!(
            decode_account_set_entry_v1(&blank, 0),
            Err(Error::ZeroIdentifier)
        );
        let mut owner_only = blank;
        put(&mut owner_only, 0, &[1u8; 32]).expect("key");
        assert_eq!(
            decode_account_set_entry_v1(&owner_only, 0),
            Err(Error::ZeroIdentifier)
        );
    }

    #[test]
    fn an_empty_or_oversized_set_has_no_preimage() {
        assert_eq!(
            account_set_id_preimage_len_v1(0),
            Err(Error::InvalidSetGeometry)
        );
        assert_eq!(
            account_set_id_preimage_len_v1(MAX_RELAYED_ACCOUNTS_V1 + 1),
            Err(Error::InvalidSetGeometry)
        );
    }

    #[test]
    fn the_set_digest_seed_is_exact() {
        let mut buffer = [0u8; SET_DIGEST_SEED_PREIMAGE_BYTES];
        let written =
            encode_set_digest_seed_preimage_v1(&mut buffer, [6; 32], 423_941_138).expect("encode");
        assert_eq!(written, SET_DIGEST_SEED_PREIMAGE_BYTES);
        assert_eq!(buffer.get(..22), Some(RELAYED_SET_DIGEST_DOMAIN_V1));
        assert_eq!(buffer.get(22), Some(&0));
        let mut short = [0u8; SET_DIGEST_SEED_PREIMAGE_BYTES - 1];
        assert_eq!(
            encode_set_digest_seed_preimage_v1(&mut short, [6; 32], 1),
            Err(Error::OutputLength)
        );
    }
}
