//! A Pyth publication the lab can mint at its own hour.
//!
//! # The fixture question this answers
//!
//! `fixtures/pyth/local-upgraded-2026-08-22` carries ONE signed VAA and ONE
//! `PostUpdate` body, both frozen at the capture date, and every loopback
//! market's window ends at that frozen publication. The ladder tier measured
//! what that costs: one `WindowSpecV1.max_age_seconds` governs both the
//! crank's admissibility (`window.end + max_age` is the primary leg's deadline)
//! and the publication's freshness (`[now - max_age, now + skew]`), so a shelf
//! life short enough for the primary leg to close inside a run is one under
//! which the frozen publication is already stale for every rung after it. No
//! value of that field satisfies both legs against a frozen capture; a rung
//! answered on a loopback needs a publication the lab can REFRESH.
//!
//! This module refreshes it. The lab's guardian set is the nineteen dummy
//! guardians the pinned upstream test utilities derive from
//! `secret_i = [i + 1, 0, ..., 0]` -- public test material, not a key anyone
//! holds -- and `fixtures/pyth/local-upgraded-2026-08-22/guardian-set-0.account.hex`
//! was checked against exactly that derivation before this file was written
//! (nineteen of nineteen addresses; the fixture's thirteen signatures recover
//! to their guardians under the double keccak of the body; the fixture's
//! single-leaf root is `keccak256(0x00 ‖ message)[..20]`). So a fresh VAA over a
//! fresh `PriceFeedMessage` is signable here, offline, at any instant the
//! caller names, and the real router and receiver ELFs verify it exactly as
//! they verify the capture: thirteen of nineteen secp256k1 signatures, then one
//! Merkle proof against the root the VAA carries.
//!
//! # What is a lab shape, stated plainly
//!
//! The publication is authored by the lab and signed by keys everyone can
//! derive. It proves the WIRE -- the router's verification, the receiver's
//! posting, the Resolution program's admission -- and nothing about any price.
//! The shelf life a caller states is the LAB's tolerance for the age of the
//! publication it minted, never a market's staleness policy; that is why it is
//! a parameter of this producer and of the ladder tier, and not a field of the
//! market shape.
//!
//! # One author for every byte
//!
//! The message, the leaf, the root, the payload, the body, the digest and the
//! signature entry are each built by one function below and the tests
//! reproduce the pinned fixture's root and payload from the fixture's own
//! message, so a producer that drifted from the upstream wire would fail
//! against the capture before it ever met a validator.

use libsecp256k1::{Message, PublicKey, SecretKey};
use solana_program::keccak;

use crate::{Error, Result};

/// The number of guardians in the lab's set, and the strict two-thirds quorum
/// the Wormhole router requires to verify a VAA against it.
pub(crate) const LAB_GUARDIAN_COUNT_V1: u8 = 19;
/// `(2 * 19) / 3 + 1`: the router's own quorum rule over the lab set.
pub(crate) const LAB_GUARDIAN_QUORUM_V1: u8 = 13;

/// The data source the lab receiver's `Config` admits: Wormhole chain `1` and
/// the all-ones emitter, exactly as the fixture's `receiver-initialize.data`
/// and every VAA it accepts state them.
pub(crate) const LAB_EMITTER_CHAIN_V1: u16 = 1;
pub(crate) const LAB_EMITTER_V1: [u8; 32] = [1; 32];

/// The feed the captured fixture publishes: forty-two in every byte, which is
/// the upstream test utilities' dummy feed and the identity every loopback
/// market's adapter configuration pins.
pub(crate) const LAB_FEED_ID_V1: [u8; 32] = [42; 32];

/// The captured fixture's exponent and its price, so a minted publication that
/// states nothing else lands on the scale every loopback market declares.
pub(crate) const LAB_EXPONENT_V1: i32 = -8;
pub(crate) const LAB_PRICE_V1: i64 = 100_000_000;
pub(crate) const LAB_CONFIDENCE_V1: u64 = 6_357;
/// The capture's exponential averages. Nothing downstream reads them, and they
/// are stated so a mint at the captured instant reproduces the captured
/// message byte for byte, which is the test that pins this producer to the
/// upstream wire.
pub(crate) const LAB_EMA_PRICE_V1: i64 = 99_999_000;
pub(crate) const LAB_EMA_CONFIDENCE_V1: u64 = 6_400;

/// The Wormhole guardian set index the lab router initialized: zero.
const LAB_GUARDIAN_SET_INDEX_V1: u32 = 0;

/// `PriceFeedMessage` on the wire: one tag byte and eight big-endian fields.
const PRICE_FEED_MESSAGE_BYTES_V1: usize = 85;
const PRICE_FEED_MESSAGE_TAG_V1: u8 = 0;

/// The accumulator's Merkle leaf and node prefixes, and the twenty-byte hash
/// width the receiver compares proofs at.
const MERKLE_LEAF_PREFIX_V1: u8 = 0;
const MERKLE_HASH_BYTES_V1: usize = 20;

/// The Wormhole accumulator payload: magic, a one-byte payload tag, the
/// Pythnet slot, the ring size and the root.
const WORMHOLE_ACCUMULATOR_MAGIC_V1: &[u8; 4] = b"AUWV";
const WORMHOLE_MERKLE_PAYLOAD_TAG_V1: u8 = 0;

/// The Anchor tag the receiver dispatches `post_update` on.
const POST_UPDATE_DISCRIMINATOR_V1: [u8; 8] = [0x85, 0x5f, 0xcf, 0xaf, 0x0b, 0x4f, 0x76, 0x2c];
/// The receiver's treasury the lab `Config` names: identifier zero.
const LAB_TREASURY_ID_V1: u8 = 0;

/// Exact `PriceUpdateV2` account image width and its Anchor discriminator, so
/// the projection below is the shape `FullPriceUpdateV2::parse` admits.
const PRICE_UPDATE_V2_LEN_V1: usize = 134;
const PRICE_UPDATE_V2_DISCRIMINATOR_V1: [u8; 8] = [0x22, 0xf1, 0x23, 0x63, 0x9d, 0x7e, 0xf4, 0xcd];
const VERIFICATION_LEVEL_FULL_TAG_V1: u8 = 1;

/// One VAA signature entry: the guardian index, `r`, `s`, and the recovery id.
const VAA_SIGNATURE_ENTRY_BYTES_V1: usize = 66;
const VAA_VERSION_V1: u8 = 1;

/// What a caller states about the publication it wants minted.
///
/// Everything not here is a fact about the lab fixture and is a constant
/// above: the feed, the emitter, the guardian set, the treasury. The
/// `publish_time` is the whole point of the producer; the price, confidence and
/// exponent default to the capture's so that a market compiled against the
/// captured fixture's adapter configuration admits the fresh publication
/// without a second configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LabPublicationRequestV1 {
    /// The instant the publication is ABOUT. The window a loopback market
    /// sells ends here, and the age the shelf life bounds is measured from it.
    pub(crate) publish_time: i64,
    /// The feed's price mantissa at `exponent`.
    pub(crate) price: i64,
    /// The feed's stated confidence, in the same mantissa.
    pub(crate) confidence: u64,
    /// The feed's decimal exponent.
    pub(crate) exponent: i32,
    /// The feed's exponential average price and confidence, carried on the
    /// wire and read by nothing this tree resolves against.
    pub(crate) ema_price: i64,
    pub(crate) ema_confidence: u64,
    /// The Wormhole sequence the VAA carries. Distinct sequences make distinct
    /// VAAs over one instant, which is what lets a rung be answered by a second
    /// publication about the same period rather than by a re-post of the first.
    pub(crate) sequence: u64,
}

impl LabPublicationRequestV1 {
    /// The capture's own price facts at a caller's instant and sequence.
    pub(crate) const fn at(publish_time: i64, sequence: u64) -> Self {
        Self {
            publish_time,
            price: LAB_PRICE_V1,
            confidence: LAB_CONFIDENCE_V1,
            exponent: LAB_EXPONENT_V1,
            ema_price: LAB_EMA_PRICE_V1,
            ema_confidence: LAB_EMA_CONFIDENCE_V1,
            sequence,
        }
    }
}

/// One minted publication: every byte the real router, the real receiver and
/// the Resolution program consume, plus the account image a market compiler
/// reads the feed identity and the window's end off.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LabPublicationV1 {
    /// The request this publication was minted from.
    pub(crate) request: LabPublicationRequestV1,
    /// The 85-byte `PriceFeedMessage` the Merkle leaf hashes.
    pub(crate) message: Vec<u8>,
    /// The twenty-byte accumulator root the VAA carries: for a single-leaf
    /// tree, the leaf hash itself.
    pub(crate) root: [u8; MERKLE_HASH_BYTES_V1],
    /// The complete signed VAA: version, guardian set index, thirteen
    /// signature entries, and the body.
    pub(crate) signed_vaa: Vec<u8>,
    /// The receiver's `PostUpdateParams` body WITHOUT the Anchor tag: exactly
    /// what `ProviderSubmitIntentV3::post_update_body` and the Resolution
    /// program's `PostUpdateParamsView::parse` take.
    pub(crate) post_update_body: Vec<u8>,
    /// The projected `PriceUpdateV2` account image the receiver will write,
    /// under the write authority the caller names. A market compiler reads the
    /// feed identity, the exponent and `publish_time` off this; the chain's
    /// own bytes are what a submission digests.
    pub(crate) projected_price_update: Vec<u8>,
}

impl LabPublicationV1 {
    /// The `post_update` instruction data: the Anchor tag and the body.
    pub(crate) fn post_update_data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(8 + self.post_update_body.len());
        data.extend_from_slice(&POST_UPDATE_DISCRIMINATOR_V1);
        data.extend_from_slice(&self.post_update_body);
        data
    }

    /// The VAA body alone, for a caller that wants to re-verify the signatures.
    pub(crate) fn vaa_body(&self) -> &[u8] {
        let header = 6 + usize::from(LAB_GUARDIAN_QUORUM_V1) * VAA_SIGNATURE_ENTRY_BYTES_V1;
        self.signed_vaa.get(header..).unwrap_or(&[])
    }
}

/// The lab guardian `index`'s secret: `[index + 1, 0, ..., 0]`.
///
/// This is the pinned upstream test utilities' derivation and nothing else; it
/// is the reason a lab can sign at all, and the reason no production release
/// row can ever be satisfied by it -- a guardian set whose keys are derivable
/// is a guardian set the release catalog refuses to name.
pub(crate) fn lab_guardian_secret_v1(index: u8) -> Result<SecretKey> {
    if index >= LAB_GUARDIAN_COUNT_V1 {
        return Err(Error::new(format!(
            "lab guardian {index} does not exist: the set has {LAB_GUARDIAN_COUNT_V1} members"
        )));
    }
    let mut bytes = [0_u8; 32];
    bytes[0] = index
        .checked_add(1)
        .ok_or_else(|| Error::new("lab guardian index overflowed"))?;
    SecretKey::parse(&bytes).map_err(|error| Error::new(format!("lab guardian secret: {error:?}")))
}

/// The twenty-byte Ethereum-style address of one guardian: the keccak of the
/// uncompressed public key without its `0x04` prefix, last twenty bytes.
pub(crate) fn lab_guardian_address_v1(index: u8) -> Result<[u8; 20]> {
    let secret = lab_guardian_secret_v1(index)?;
    let public = PublicKey::from_secret_key(&secret).serialize();
    let digest = keccak::hash(
        public
            .get(1..)
            .ok_or_else(|| Error::new("uncompressed public key is narrower than one byte"))?,
    )
    .to_bytes();
    let mut address = [0_u8; 20];
    address.copy_from_slice(
        digest
            .get(12..)
            .ok_or_else(|| Error::new("keccak digest is narrower than twelve bytes"))?,
    );
    Ok(address)
}

/// Encode one `PriceFeedMessage` exactly as the accumulator serializes it.
fn price_feed_message_v1(request: LabPublicationRequestV1) -> Result<Vec<u8>> {
    if request.publish_time <= 0 {
        return Err(Error::new(
            "a publication must be about a positive instant; the Resolution program refuses a \
             non-positive publish_time by name",
        ));
    }
    let prev_publish_time = request
        .publish_time
        .checked_sub(1)
        .ok_or_else(|| Error::new("previous publish time underflowed"))?;
    let mut message = Vec::with_capacity(PRICE_FEED_MESSAGE_BYTES_V1);
    message.push(PRICE_FEED_MESSAGE_TAG_V1);
    message.extend_from_slice(&LAB_FEED_ID_V1);
    message.extend_from_slice(&request.price.to_be_bytes());
    message.extend_from_slice(&request.confidence.to_be_bytes());
    message.extend_from_slice(&request.exponent.to_be_bytes());
    message.extend_from_slice(&request.publish_time.to_be_bytes());
    message.extend_from_slice(&prev_publish_time.to_be_bytes());
    message.extend_from_slice(&request.ema_price.to_be_bytes());
    message.extend_from_slice(&request.ema_confidence.to_be_bytes());
    if message.len() != PRICE_FEED_MESSAGE_BYTES_V1 {
        return Err(Error::new(format!(
            "PriceFeedMessage encoded to {} bytes, not {PRICE_FEED_MESSAGE_BYTES_V1}",
            message.len()
        )));
    }
    Ok(message)
}

fn keccak160(parts: &[&[u8]]) -> Result<[u8; MERKLE_HASH_BYTES_V1]> {
    let digest = keccak::hashv(parts).to_bytes();
    let mut output = [0_u8; MERKLE_HASH_BYTES_V1];
    output.copy_from_slice(
        digest
            .get(..MERKLE_HASH_BYTES_V1)
            .ok_or_else(|| Error::new("keccak digest is narrower than twenty bytes"))?,
    );
    Ok(output)
}

/// The Wormhole accumulator payload over one root.
fn wormhole_payload_v1(root: &[u8; MERKLE_HASH_BYTES_V1]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + 1 + 8 + 4 + MERKLE_HASH_BYTES_V1);
    payload.extend_from_slice(WORMHOLE_ACCUMULATOR_MAGIC_V1);
    payload.push(WORMHOLE_MERKLE_PAYLOAD_TAG_V1);
    // The Pythnet slot and ring size are facts about a Pythnet the lab does not
    // run; the capture carries zero for both and the receiver reads neither.
    payload.extend_from_slice(&0_u64.to_be_bytes());
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(root);
    payload
}

/// The VAA body: timestamp, nonce, emitter chain, emitter, sequence,
/// consistency level, payload.
fn vaa_body_v1(request: LabPublicationRequestV1, payload: &[u8]) -> Result<Vec<u8>> {
    let timestamp = u32::try_from(request.publish_time)
        .map_err(|_| Error::new("the publication instant does not fit a Wormhole timestamp"))?;
    let mut body = Vec::with_capacity(4 + 4 + 2 + 32 + 8 + 1 + payload.len());
    body.extend_from_slice(&timestamp.to_be_bytes());
    body.extend_from_slice(&0_u32.to_be_bytes());
    body.extend_from_slice(&LAB_EMITTER_CHAIN_V1.to_be_bytes());
    body.extend_from_slice(&LAB_EMITTER_V1);
    body.extend_from_slice(&request.sequence.to_be_bytes());
    body.push(0);
    body.extend_from_slice(payload);
    Ok(body)
}

/// The digest every guardian signs: the keccak of the keccak of the body.
fn vaa_digest_v1(body: &[u8]) -> [u8; 32] {
    keccak::hash(keccak::hash(body).as_ref()).to_bytes()
}

/// Sign one digest with guardians `0..quorum` in index order.
///
/// The upstream test helper picks its thirteen with a thread-local RNG, which
/// is why the fixture's signature subset is not reproducible from its own
/// provenance. This producer picks the leading thirteen, so two mints of one
/// request are byte-identical and a transcript can name the VAA it posted.
fn sign_vaa_v1(digest: &[u8; 32]) -> Result<Vec<u8>> {
    let message = Message::parse(digest);
    let mut entries =
        Vec::with_capacity(usize::from(LAB_GUARDIAN_QUORUM_V1) * VAA_SIGNATURE_ENTRY_BYTES_V1);
    for index in 0..LAB_GUARDIAN_QUORUM_V1 {
        let secret = lab_guardian_secret_v1(index)?;
        let (signature, recovery) = libsecp256k1::sign(&message, &secret);
        entries.push(index);
        entries.extend_from_slice(&signature.serialize());
        entries.push(recovery.serialize());
    }
    Ok(entries)
}

/// The receiver's `PostUpdateParams` body over one message with an empty
/// proof: a single-leaf tree proves itself.
fn post_update_body_v1(message: &[u8]) -> Result<Vec<u8>> {
    let length = u32::try_from(message.len())
        .map_err(|_| Error::new("PriceFeedMessage length exceeded u32"))?;
    let mut body = Vec::with_capacity(4 + message.len() + 4 + 1);
    body.extend_from_slice(&length.to_le_bytes());
    body.extend_from_slice(message);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.push(LAB_TREASURY_ID_V1);
    Ok(body)
}

/// Project the `PriceUpdateV2` account the receiver writes for this message.
///
/// `posted_slot` is the chain's and is unknown at mint time; the projection
/// carries zero, and every consumer of the projection reads only the feed
/// identity, the exponent and the instant. A submission digests the CHAIN's
/// bytes, never these.
fn projected_price_update_v1(
    request: LabPublicationRequestV1,
    write_authority: [u8; 32],
) -> Result<Vec<u8>> {
    let mut image = Vec::with_capacity(PRICE_UPDATE_V2_LEN_V1);
    image.extend_from_slice(&PRICE_UPDATE_V2_DISCRIMINATOR_V1);
    image.extend_from_slice(&write_authority);
    image.push(VERIFICATION_LEVEL_FULL_TAG_V1);
    image.extend_from_slice(&LAB_FEED_ID_V1);
    image.extend_from_slice(&request.price.to_le_bytes());
    image.extend_from_slice(&request.confidence.to_le_bytes());
    image.extend_from_slice(&request.exponent.to_le_bytes());
    image.extend_from_slice(&request.publish_time.to_le_bytes());
    image.extend_from_slice(
        &request
            .publish_time
            .checked_sub(1)
            .ok_or_else(|| Error::new("previous publish time underflowed"))?
            .to_le_bytes(),
    );
    image.extend_from_slice(&request.ema_price.to_le_bytes());
    image.extend_from_slice(&request.ema_confidence.to_le_bytes());
    image.extend_from_slice(&0_u64.to_le_bytes());
    image.push(0);
    if image.len() != PRICE_UPDATE_V2_LEN_V1 {
        return Err(Error::new(format!(
            "PriceUpdateV2 projected to {} bytes, not {PRICE_UPDATE_V2_LEN_V1}",
            image.len()
        )));
    }
    Ok(image)
}

/// Mint one publication.
///
/// `write_authority` is the key the projected account image names; the chain's
/// own image names whatever authority the Resolution submit route derives, and
/// a market compiler reads only the feed facts off the projection.
pub(crate) fn mint_lab_publication_v1(
    request: LabPublicationRequestV1,
    write_authority: [u8; 32],
) -> Result<LabPublicationV1> {
    let message = price_feed_message_v1(request)?;
    let root = keccak160(&[&[MERKLE_LEAF_PREFIX_V1], &message])?;
    let payload = wormhole_payload_v1(&root);
    let body = vaa_body_v1(request, &payload)?;
    let signatures = sign_vaa_v1(&vaa_digest_v1(&body))?;
    let mut signed_vaa = Vec::with_capacity(6 + signatures.len() + body.len());
    signed_vaa.push(VAA_VERSION_V1);
    signed_vaa.extend_from_slice(&LAB_GUARDIAN_SET_INDEX_V1.to_be_bytes());
    signed_vaa.push(LAB_GUARDIAN_QUORUM_V1);
    signed_vaa.extend_from_slice(&signatures);
    signed_vaa.extend_from_slice(&body);
    Ok(LabPublicationV1 {
        request,
        post_update_body: post_update_body_v1(&message)?,
        projected_price_update: projected_price_update_v1(request, write_authority)?,
        message,
        root,
        signed_vaa,
    })
}

/// Recover every signature entry of a signed VAA and name the guardian it
/// belongs to, in entry order.
///
/// The router does exactly this on chain; doing it here lets a transcript say
/// which guardians a posted VAA carried and lets the tests below prove the
/// producer signs what the router verifies.
pub(crate) fn recover_vaa_signers_v1(signed_vaa: &[u8]) -> Result<Vec<u8>> {
    let count = usize::from(
        *signed_vaa
            .get(5)
            .ok_or_else(|| Error::new("the VAA is narrower than its header"))?,
    );
    let header = 6 + count * VAA_SIGNATURE_ENTRY_BYTES_V1;
    let body = signed_vaa
        .get(header..)
        .ok_or_else(|| Error::new("the VAA is narrower than its signature entries"))?;
    let message = Message::parse(&vaa_digest_v1(body));
    let mut signers = Vec::with_capacity(count);
    for entry in 0..count {
        let start = 6 + entry * VAA_SIGNATURE_ENTRY_BYTES_V1;
        let bytes = signed_vaa
            .get(start..start + VAA_SIGNATURE_ENTRY_BYTES_V1)
            .ok_or_else(|| Error::new("a signature entry is truncated"))?;
        let index = bytes[0];
        let signature = libsecp256k1::Signature::parse_standard_slice(&bytes[1..65])
            .map_err(|error| Error::new(format!("signature entry {entry}: {error:?}")))?;
        let recovery = libsecp256k1::RecoveryId::parse(bytes[65])
            .map_err(|error| Error::new(format!("recovery id of entry {entry}: {error:?}")))?;
        let public = libsecp256k1::recover(&message, &signature, &recovery)
            .map_err(|error| Error::new(format!("recovering entry {entry}: {error:?}")))?;
        let digest = keccak::hash(&public.serialize()[1..]).to_bytes();
        let mut address = [0_u8; 20];
        address.copy_from_slice(&digest[12..]);
        if address != lab_guardian_address_v1(index)? {
            return Err(Error::new(format!(
                "signature entry {entry} names guardian {index} and recovers to another address"
            )));
        }
        signers.push(index);
    }
    Ok(signers)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_GUARDIAN_SET: &str = include_str!(
        "../../../../../fixtures/pyth/local-upgraded-2026-08-22/guardian-set-0.account.hex"
    );
    const FIXTURE_SIGNED_VAA: &[u8] =
        include_bytes!("../../../../../fixtures/pyth/local-upgraded-2026-08-22/signed.vaa");
    const FIXTURE_POST_UPDATE: &[u8] = include_bytes!(
        "../../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data"
    );
    const FIXTURE_PRICE_UPDATE: &[u8] = include_bytes!(
        "../../../../../fixtures/pyth/local-upgraded-2026-08-22/price-update.account"
    );

    fn fixture_guardians() -> Vec<[u8; 20]> {
        let bytes: Vec<u8> = (0..FIXTURE_GUARDIAN_SET.trim().len() / 2)
            .map(|index| {
                u8::from_str_radix(&FIXTURE_GUARDIAN_SET.trim()[2 * index..2 * index + 2], 16)
                    .expect("hex")
            })
            .collect();
        let count = u32::from_le_bytes(bytes[4..8].try_into().expect("count")) as usize;
        (0..count)
            .map(|index| {
                bytes[8 + 20 * index..8 + 20 * index + 20]
                    .try_into()
                    .expect("address")
            })
            .collect()
    }

    /// The lab guardian set IS the derivable one: every address the router
    /// was initialized with is `keccak(pubkey([i + 1, 0..]))[12..]`.
    #[test]
    fn the_fixture_guardian_set_is_the_derivable_dummy_set() {
        let addresses = fixture_guardians();
        assert_eq!(addresses.len(), usize::from(LAB_GUARDIAN_COUNT_V1));
        for (index, expected) in addresses.iter().enumerate() {
            assert_eq!(
                &lab_guardian_address_v1(index as u8).expect("address"),
                expected,
                "guardian {index}"
            );
        }
    }

    /// The captured VAA's thirteen signatures recover to the guardians they
    /// name under this module's digest rule, so the rule is the router's.
    #[test]
    fn the_captured_vaa_recovers_under_the_double_keccak() {
        let signers = recover_vaa_signers_v1(FIXTURE_SIGNED_VAA).expect("captured VAA recovers");
        assert_eq!(signers.len(), usize::from(LAB_GUARDIAN_QUORUM_V1));
        assert_eq!(signers, vec![1, 2, 3, 5, 6, 7, 8, 10, 11, 13, 15, 16, 18]);
    }

    /// The captured message's single-leaf root is the one the captured VAA
    /// carries, so the leaf rule and the payload layout are the receiver's.
    #[test]
    fn the_captured_root_is_the_leaf_hash_of_the_captured_message() {
        let body = &FIXTURE_POST_UPDATE[8..];
        let length = u32::from_le_bytes(body[..4].try_into().expect("length")) as usize;
        let message = &body[4..4 + length];
        assert_eq!(length, PRICE_FEED_MESSAGE_BYTES_V1);
        let root = keccak160(&[&[MERKLE_LEAF_PREFIX_V1], message]).expect("leaf");
        let vaa_body = &FIXTURE_SIGNED_VAA[6 + 13 * VAA_SIGNATURE_ENTRY_BYTES_V1..];
        let payload = &vaa_body[51..];
        assert_eq!(payload, wormhole_payload_v1(&root).as_slice());
        assert_eq!(&vaa_body[8..10], &LAB_EMITTER_CHAIN_V1.to_be_bytes());
        assert_eq!(&vaa_body[10..42], &LAB_EMITTER_V1);
    }

    /// A mint at the capture's own instant reproduces the capture's message
    /// and root byte for byte; only the signatures (a different thirteen) and
    /// the sequence differ, and the fresh signatures recover to the set.
    #[test]
    fn a_mint_at_the_captured_instant_reproduces_the_captured_message_and_root() {
        let captured = dclutch_source::pyth::FullPriceUpdateV2::parse(FIXTURE_PRICE_UPDATE)
            .expect("captured update");
        let request = LabPublicationRequestV1 {
            publish_time: captured.publish_time(),
            price: captured.price(),
            confidence: captured.confidence(),
            exponent: captured.exponent(),
            ema_price: captured.ema_price(),
            ema_confidence: captured.ema_confidence(),
            sequence: 2,
        };
        let minted = mint_lab_publication_v1(request, captured.write_authority()).expect("minted");
        let fixture_body = &FIXTURE_POST_UPDATE[8..];
        assert_eq!(minted.post_update_body, fixture_body);
        assert_eq!(minted.post_update_data(), FIXTURE_POST_UPDATE);
        let captured_body = &FIXTURE_SIGNED_VAA[6 + 13 * VAA_SIGNATURE_ENTRY_BYTES_V1..];
        // The capture's body has a zero timestamp; the mint stamps the
        // instant. Everything after the timestamp is byte-identical.
        assert_eq!(&minted.vaa_body()[4..], &captured_body[4..]);
        let signers = recover_vaa_signers_v1(&minted.signed_vaa).expect("fresh VAA recovers");
        assert_eq!(signers, (0..LAB_GUARDIAN_QUORUM_V1).collect::<Vec<u8>>());
        let projected =
            dclutch_source::pyth::FullPriceUpdateV2::parse(&minted.projected_price_update)
                .expect("the projection parses as a full update");
        assert_eq!(projected.feed_id(), captured.feed_id());
        assert_eq!(projected.exponent(), captured.exponent());
        assert_eq!(projected.publish_time(), captured.publish_time());
        assert_eq!(projected.price(), captured.price());
    }

    /// Two mints of one request are one publication; two sequences are two.
    #[test]
    fn a_mint_is_deterministic_and_a_sequence_distinguishes_it() {
        let first = mint_lab_publication_v1(LabPublicationRequestV1::at(1_800_000_000, 7), [9; 32])
            .expect("first");
        let again = mint_lab_publication_v1(LabPublicationRequestV1::at(1_800_000_000, 7), [9; 32])
            .expect("again");
        assert_eq!(first, again);
        let other = mint_lab_publication_v1(LabPublicationRequestV1::at(1_800_000_000, 8), [9; 32])
            .expect("other sequence");
        assert_eq!(
            other.message, first.message,
            "the message is about the instant"
        );
        assert_ne!(
            other.signed_vaa, first.signed_vaa,
            "the VAA carries the sequence"
        );
    }

    /// A non-positive instant is refused here rather than on chain.
    #[test]
    fn a_non_positive_instant_refuses_at_the_mint() {
        let refusal = mint_lab_publication_v1(LabPublicationRequestV1::at(0, 1), [9; 32])
            .expect_err("zero instant");
        assert!(
            refusal.to_string().contains("positive instant"),
            "{refusal}"
        );
    }
}
