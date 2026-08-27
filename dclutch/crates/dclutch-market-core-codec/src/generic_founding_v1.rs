//! Family-neutral atomic Market founding request and acknowledgement.
//!
//! The request is an immutable capability-artifact projection.  It carries
//! only the coordinates that cannot be recovered from Found31 or the
//! projected-Custody state.  Core still authenticates every repeated field
//! against those semantic owners before creating a Market or a permit.

use crate::{Error, Identity};
use sha2::{Digest, Sha256};

/// Exact generic-founding request magic.
pub const GENERIC_FOUNDING_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTGFQ1";
/// Exact generic-founding acknowledgement magic.
pub const GENERIC_FOUNDING_ACK_MAGIC_V1: [u8; 8] = *b"DCLTGFA1";
/// Exact fixed request width.
pub const GENERIC_FOUNDING_REQUEST_BYTES_V1: usize = 400;
/// Exact fixed acknowledgement width.
pub const GENERIC_FOUNDING_ACK_BYTES_V1: usize = 248;
/// Fixed Core accounts before the exact ordered FundingState span.
pub const GENERIC_FOUNDING_FOUND_FIXED_ACCOUNT_COUNT_V1: usize = 34;
/// Fixed Core accounts after the exact ordered FundingState span.
pub const GENERIC_FOUNDING_FOUND_SUFFIX_ACCOUNT_COUNT_V1: usize = 15;
/// Exact Core final-Open account count.
pub const GENERIC_FOUNDING_OPEN_ACCOUNT_COUNT_V1: usize = 23;
/// Domain for the exact ordered generic FundingState account list.
pub const GENERIC_FOUNDING_FUNDING_LIST_DOMAIN_V1: &[u8] =
    b"dclutch/generic-founding-funding-list/v1";
/// Domain for the Core Found-and-permit post-resource commitment.
pub const GENERIC_FOUNDING_FOUND_POST_RESOURCE_DOMAIN_V1: &[u8] =
    b"dclutch/generic-founding-found-post-resource/v1";
/// Domain for the Core final-Open post-resource commitment.
pub const GENERIC_FOUNDING_OPEN_POST_RESOURCE_DOMAIN_V1: &[u8] =
    b"dclutch/generic-founding-open-post-resource/v1";
/// Maximum FundingState count admitted by the first physical profile.
pub const GENERIC_FOUNDING_MAX_FUNDING_STATES_V1: usize = 16;

const VERSION_V1: u16 = 1;
const REQUEST_IDENTITIES_OFFSET: usize = 16;
const REQUEST_CAPABILITY_ROOT_OFFSET: usize = REQUEST_IDENTITIES_OFFSET + 2 * 32;
const REQUEST_GENERATION_OFFSET: usize = 336;
const REQUEST_ENTRY_INDEX_OFFSET: usize = 392;
const REQUEST_TAIL_RESERVED_OFFSET: usize = 394;
const REQUEST_TAIL_RESERVED_BYTES: usize = 6;
const ACK_IDENTITIES_OFFSET: usize = 16;
const ACK_GENERATION_OFFSET: usize = 240;

/// Hash one exact ordered, nonempty, alias-free FundingState address list.
///
/// The canonical preimage is `domain || 0 || u16_le(count) || key...`.
pub fn generic_founding_funding_list_id_v1(funding_states: &[Identity]) -> Result<Identity, Error> {
    if funding_states.is_empty() || funding_states.len() > GENERIC_FOUNDING_MAX_FUNDING_STATES_V1 {
        return Err(Error::InvalidCoordinates);
    }
    let count = u16::try_from(funding_states.len()).map_err(|_| Error::InvalidCoordinates)?;
    let mut hasher = Sha256::new();
    hasher.update(GENERIC_FOUNDING_FUNDING_LIST_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(count.to_le_bytes());
    for (index, key) in funding_states.iter().enumerate() {
        if funding_states
            .get(..index)
            .ok_or(Error::InvalidLength)?
            .iter()
            .any(|prior| prior == key)
        {
            return Err(Error::InvalidCoordinates);
        }
        hasher.update(key.to_bytes());
    }
    Identity::new(hasher.finalize().into())
}

/// Stage of the atomic generic founding protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GenericFoundingStageV1 {
    /// Create the authenticated Founding Market and one-shot Claims permit.
    FoundAndPermit = 1,
    /// Consume authenticated Claims poststate and commit the Market Open last.
    Open = 2,
}

impl GenericFoundingStageV1 {
    fn decode(value: u8) -> Result<Self, Error> {
        match value {
            1 => Ok(Self::FoundAndPermit),
            2 => Ok(Self::Open),
            _ => Err(Error::InvalidCoordinates),
        }
    }
}

/// Artifact-owned coordinates shared by the Found and final-Open stages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericFoundingRequestV1 {
    stage: GenericFoundingStageV1,
    funding_count: u8,
    release_set: Identity,
    market: Identity,
    capability_root: Identity,
    context: Identity,
    founder: Identity,
    beneficiary: Identity,
    funding_source: Identity,
    hoard: Identity,
    projected_replay: Identity,
    funding_list_id: Identity,
    generation: u64,
    quantity: u64,
    basis_scale: u64,
    expiry_slot: u64,
    market_rent: u64,
    permit_rent: u64,
    projected_resulting_revision: u64,
    capability_entry_index: u16,
}

impl GenericFoundingRequestV1 {
    /// Construct one canonical request from an authenticated artifact.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        stage: GenericFoundingStageV1,
        funding_count: u8,
        release_set: Identity,
        market: Identity,
        capability_root: Identity,
        context: Identity,
        founder: Identity,
        beneficiary: Identity,
        funding_source: Identity,
        hoard: Identity,
        projected_replay: Identity,
        funding_list_id: Identity,
        generation: u64,
        quantity: u64,
        basis_scale: u64,
        expiry_slot: u64,
        market_rent: u64,
        permit_rent: u64,
        projected_resulting_revision: u64,
        capability_entry_index: u16,
    ) -> Result<Self, Error> {
        let value = Self {
            stage,
            funding_count,
            release_set,
            market,
            capability_root,
            context,
            founder,
            beneficiary,
            funding_source,
            hoard,
            projected_replay,
            funding_list_id,
            generation,
            quantity,
            basis_scale,
            expiry_slot,
            market_rent,
            permit_rent,
            projected_resulting_revision,
            capability_entry_index,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<(), Error> {
        if self.funding_count == 0
            || usize::from(self.funding_count) > GENERIC_FOUNDING_MAX_FUNDING_STATES_V1
            || self.generation == 0
            || self.quantity == 0
            || self.basis_scale == 0
            || self.expiry_slot == 0
            || self.market_rent == 0
            || self.permit_rent == 0
            || self.projected_resulting_revision < 2
            || self.quantity.checked_mul(self.basis_scale).is_none()
            || self.capability_root == self.context
            || self.funding_source == self.hoard
            || self.projected_replay == self.hoard
            || self.projected_replay == self.funding_source
        {
            return Err(Error::InvalidCoordinates);
        }
        Ok(())
    }

    /// Hostile-decode one exact fixed request.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_header(
            input,
            GENERIC_FOUNDING_REQUEST_BYTES_V1,
            &GENERIC_FOUNDING_REQUEST_MAGIC_V1,
        )?;
        if read_u16(input, 8)? != VERSION_V1
            || nonzero(input, 12, 4)?
            || nonzero(
                input,
                REQUEST_TAIL_RESERVED_OFFSET,
                REQUEST_TAIL_RESERVED_BYTES,
            )?
        {
            return Err(Error::NonzeroReserved);
        }
        let ids = read_identities::<10>(input, REQUEST_IDENTITIES_OFFSET)?;
        Self::new(
            GenericFoundingStageV1::decode(read_u8(input, 10)?)?,
            read_u8(input, 11)?,
            ids[0],
            ids[1],
            ids[2],
            ids[3],
            ids[4],
            ids[5],
            ids[6],
            ids[7],
            ids[8],
            ids[9],
            read_u64(input, REQUEST_GENERATION_OFFSET)?,
            read_u64(input, REQUEST_GENERATION_OFFSET + 8)?,
            read_u64(input, REQUEST_GENERATION_OFFSET + 16)?,
            read_u64(input, REQUEST_GENERATION_OFFSET + 24)?,
            read_u64(input, REQUEST_GENERATION_OFFSET + 32)?,
            read_u64(input, REQUEST_GENERATION_OFFSET + 40)?,
            read_u64(input, REQUEST_GENERATION_OFFSET + 48)?,
            read_u16(input, REQUEST_ENTRY_INDEX_OFFSET)?,
        )
    }

    /// Encode the sole canonical fixed request.
    pub fn encode(self) -> Result<[u8; GENERIC_FOUNDING_REQUEST_BYTES_V1], Error> {
        self.validate()?;
        let mut output = [0; GENERIC_FOUNDING_REQUEST_BYTES_V1];
        put(&mut output, 0, &GENERIC_FOUNDING_REQUEST_MAGIC_V1)?;
        put(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        put(&mut output, 10, &[self.stage as u8, self.funding_count])?;
        put_identities(
            &mut output,
            REQUEST_IDENTITIES_OFFSET,
            &[
                self.release_set,
                self.market,
                self.capability_root,
                self.context,
                self.founder,
                self.beneficiary,
                self.funding_source,
                self.hoard,
                self.projected_replay,
                self.funding_list_id,
            ],
        )?;
        for (index, value) in [
            self.generation,
            self.quantity,
            self.basis_scale,
            self.expiry_slot,
            self.market_rent,
            self.permit_rent,
            self.projected_resulting_revision,
        ]
        .into_iter()
        .enumerate()
        {
            put(
                &mut output,
                REQUEST_GENERATION_OFFSET + index * 8,
                &value.to_le_bytes(),
            )?;
        }
        put(
            &mut output,
            REQUEST_ENTRY_INDEX_OFFSET,
            &self.capability_entry_index.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Return the selected protocol stage.
    pub const fn stage(self) -> GenericFoundingStageV1 {
        self.stage
    }
    /// Return the exact ordered FundingState count.
    pub const fn funding_count(self) -> u8 {
        self.funding_count
    }
    /// Return the selected release set.
    pub const fn release_set(self) -> Identity {
        self.release_set
    }
    /// Return the derived Market.
    pub const fn market(self) -> Identity {
        self.market
    }
    /// Return the immutable Trading capability root.
    pub const fn capability_root(self) -> Identity {
        self.capability_root
    }
    /// Return the artifact-owned action context.
    pub const fn context(self) -> Identity {
        self.context
    }
    /// Return the founder receiving complete-set Claims.
    pub const fn founder(self) -> Identity {
        self.founder
    }
    /// Return the permanent rent beneficiary.
    pub const fn beneficiary(self) -> Identity {
        self.beneficiary
    }
    /// Return the exact source Vault consumed by projected Custody.
    pub const fn funding_source(self) -> Identity {
        self.funding_source
    }
    /// Return the exact resulting Hoard Vault.
    pub const fn hoard(self) -> Identity {
        self.hoard
    }
    /// Return the projected replay rewritten into canonical Custody replay.
    pub const fn projected_replay(self) -> Identity {
        self.projected_replay
    }
    /// Return the exact ordered FundingState-list identity.
    pub const fn funding_list_id(self) -> Identity {
        self.funding_list_id
    }
    /// Return the Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Return the positive complete-set quantity.
    pub const fn quantity(self) -> u64 {
        self.quantity
    }
    /// Return the exact Product basis scale.
    pub const fn basis_scale(self) -> u64 {
        self.basis_scale
    }
    /// Return the last admitted founding slot.
    pub const fn expiry_slot(self) -> u64 {
        self.expiry_slot
    }
    /// Return the exact prepaid Market rent.
    pub const fn market_rent(self) -> u64 {
        self.market_rent
    }
    /// Return the exact prepaid one-shot permit rent.
    pub const fn permit_rent(self) -> u64 {
        self.permit_rent
    }
    /// Return the projected-Custody terminal revision.
    pub const fn projected_resulting_revision(self) -> u64 {
        self.projected_resulting_revision
    }
    /// Return the selected Market capability-manifest entry index.
    ///
    /// Core reads the Market's own authenticated capability manifest at this
    /// index and takes the entry's exact kind and capability release. The
    /// index is not independently bounded: the authenticated manifest's actual
    /// entry count is the sole bound, exactly as in Decision 0003.
    pub const fn capability_entry_index(self) -> u16 {
        self.capability_entry_index
    }
    /// Return exact Hoard principal with checked multiplication.
    pub fn hoard_principal(self) -> Result<u64, Error> {
        self.quantity
            .checked_mul(self.basis_scale)
            .ok_or(Error::InvalidCoordinates)
    }

    /// Encode the canonical root-free capability-selection preimage.
    ///
    /// The Trading capability-root PDA commits to the selected capability
    /// config identity, and the selected config identity is the digest of this
    /// preimage. A preimage that carried the root address back would therefore
    /// demand a SHA-256 fixed point and no honest founder could ever produce
    /// one. This projection pins the Found-and-permit stage and clears only the
    /// capability-root identity; every other artifact coordinate stays bound
    /// byte-for-byte, so the root address still commits to the exact Market,
    /// generation, release set, context, vaults, funding list, widths, rents,
    /// and revision.
    pub fn selection_preimage(self) -> Result<[u8; GENERIC_FOUNDING_REQUEST_BYTES_V1], Error> {
        let mut bytes = self
            .with_stage(GenericFoundingStageV1::FoundAndPermit)?
            .encode()?;
        put(&mut bytes, REQUEST_CAPABILITY_ROOT_OFFSET, &[0; 32])?;
        Ok(bytes)
    }

    /// Return the exact selected capability config identity for this request.
    ///
    /// This is the sole value a Trading capability root may carry as its
    /// selection config when the root authorizes atomic generic founding.
    pub fn selection_config_id(self) -> Result<Identity, Error> {
        Identity::new(Sha256::digest(self.selection_preimage()?).into())
    }

    /// Return the same artifact coordinates for the other atomic stage.
    pub fn with_stage(self, stage: GenericFoundingStageV1) -> Result<Self, Error> {
        Self::new(
            stage,
            self.funding_count,
            self.release_set,
            self.market,
            self.capability_root,
            self.context,
            self.founder,
            self.beneficiary,
            self.funding_source,
            self.hoard,
            self.projected_replay,
            self.funding_list_id,
            self.generation,
            self.quantity,
            self.basis_scale,
            self.expiry_slot,
            self.market_rent,
            self.permit_rent,
            self.projected_resulting_revision,
            self.capability_entry_index,
        )
    }
}

/// Core-produced acknowledgement of one generic founding stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericFoundingAckV1 {
    stage: GenericFoundingStageV1,
    funding_count: u8,
    core_program: Identity,
    release_set: Identity,
    market: Identity,
    permit: Identity,
    request_digest: Identity,
    post_resource_digest: Identity,
    funding_list_id: Identity,
    generation: u64,
}

impl GenericFoundingAckV1 {
    /// Construct an acknowledgement from Core-authenticated facts.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        request: GenericFoundingRequestV1,
        core_program: Identity,
        permit: Identity,
        request_digest: Identity,
        post_resource_digest: Identity,
    ) -> Self {
        Self {
            stage: request.stage,
            funding_count: request.funding_count,
            core_program,
            release_set: request.release_set,
            market: request.market,
            permit,
            request_digest,
            post_resource_digest,
            funding_list_id: request.funding_list_id,
            generation: request.generation,
        }
    }

    /// Hostile-decode one exact acknowledgement.
    pub fn decode(input: &[u8]) -> Result<Self, Error> {
        exact_header(
            input,
            GENERIC_FOUNDING_ACK_BYTES_V1,
            &GENERIC_FOUNDING_ACK_MAGIC_V1,
        )?;
        if read_u16(input, 8)? != VERSION_V1 || nonzero(input, 12, 4)? {
            return Err(Error::NonzeroReserved);
        }
        let ids = read_identities::<7>(input, ACK_IDENTITIES_OFFSET)?;
        let funding_count = read_u8(input, 11)?;
        let generation = read_u64(input, ACK_GENERATION_OFFSET)?;
        if funding_count == 0 || generation == 0 {
            return Err(Error::InvalidCoordinates);
        }
        Ok(Self {
            stage: GenericFoundingStageV1::decode(read_u8(input, 10)?)?,
            funding_count,
            core_program: ids[0],
            release_set: ids[1],
            market: ids[2],
            permit: ids[3],
            request_digest: ids[4],
            post_resource_digest: ids[5],
            funding_list_id: ids[6],
            generation,
        })
    }

    /// Encode the sole canonical acknowledgement.
    pub fn encode(self) -> Result<[u8; GENERIC_FOUNDING_ACK_BYTES_V1], Error> {
        if self.funding_count == 0 || self.generation == 0 {
            return Err(Error::InvalidCoordinates);
        }
        let mut output = [0; GENERIC_FOUNDING_ACK_BYTES_V1];
        put(&mut output, 0, &GENERIC_FOUNDING_ACK_MAGIC_V1)?;
        put(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        put(&mut output, 10, &[self.stage as u8, self.funding_count])?;
        put_identities(
            &mut output,
            ACK_IDENTITIES_OFFSET,
            &[
                self.core_program,
                self.release_set,
                self.market,
                self.permit,
                self.request_digest,
                self.post_resource_digest,
                self.funding_list_id,
            ],
        )?;
        put(
            &mut output,
            ACK_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Verify exact echo and Core post-resource evidence.
    pub fn validate_for(
        self,
        request: GenericFoundingRequestV1,
        core_program: Identity,
        permit: Identity,
        request_digest: Identity,
        post_resource_digest: Identity,
    ) -> Result<(), Error> {
        if self
            == Self::new(
                request,
                core_program,
                permit,
                request_digest,
                post_resource_digest,
            )
        {
            Ok(())
        } else {
            Err(Error::InvalidRelease)
        }
    }

    /// Return the acknowledged stage.
    pub const fn stage(self) -> GenericFoundingStageV1 {
        self.stage
    }
    /// Return the exact FundingState count.
    pub const fn funding_count(self) -> u8 {
        self.funding_count
    }
    /// Return the selected Core program.
    pub const fn core_program(self) -> Identity {
        self.core_program
    }
    /// Return the selected release set.
    pub const fn release_set(self) -> Identity {
        self.release_set
    }
    /// Return the Market.
    pub const fn market(self) -> Identity {
        self.market
    }
    /// Return the one-shot permit.
    pub const fn permit(self) -> Identity {
        self.permit
    }
    /// Return the exact request digest.
    pub const fn request_digest(self) -> Identity {
        self.request_digest
    }
    /// Return the Core post-resource digest.
    pub const fn post_resource_digest(self) -> Identity {
        self.post_resource_digest
    }
    /// Return the ordered FundingState-list identity.
    pub const fn funding_list_id(self) -> Identity {
        self.funding_list_id
    }
    /// Return the Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
}

fn exact_header(input: &[u8], width: usize, magic: &[u8; 8]) -> Result<(), Error> {
    if input.len() != width {
        return Err(Error::InvalidLength);
    }
    if input.get(..8) != Some(magic.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    Ok(())
}

fn nonzero(input: &[u8], offset: usize, width: usize) -> Result<bool, Error> {
    Ok(input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0))
}

fn read_u8(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], Error> {
    input
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_identities<const N: usize>(input: &[u8], offset: usize) -> Result<[Identity; N], Error> {
    let mut result = [Identity::new([1; 32])?; N];
    for (index, target) in result.iter_mut().enumerate() {
        *target = Identity::new(read_array(input, offset + index * 32)?)?;
    }
    Ok(result)
}

fn put(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), Error> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(bytes.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn put_identities(output: &mut [u8], offset: usize, values: &[Identity]) -> Result<(), Error> {
    for (index, value) in values.iter().enumerate() {
        put(output, offset + index * 32, &value.to_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("identity")
    }

    fn request(stage: GenericFoundingStageV1) -> GenericFoundingRequestV1 {
        GenericFoundingRequestV1::new(
            stage,
            3,
            id(1),
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
            11,
            12,
            13,
            14,
            15,
            16,
            2,
            5,
        )
        .expect("request")
    }

    #[test]
    fn request_and_ack_round_trip_and_cross_stage_refusal() {
        let found = request(GenericFoundingStageV1::FoundAndPermit);
        let found_bytes = found.encode().expect("encode");
        assert_eq!(GenericFoundingRequestV1::decode(&found_bytes), Ok(found));
        let open = found
            .with_stage(GenericFoundingStageV1::Open)
            .expect("open");
        assert_ne!(found.encode().expect("found"), open.encode().expect("open"));

        let ack = GenericFoundingAckV1::new(found, id(20), id(21), id(22), id(23));
        let bytes = ack.encode().expect("ack");
        assert_eq!(GenericFoundingAckV1::decode(&bytes), Ok(ack));
        assert_eq!(
            ack.validate_for(found, id(20), id(21), id(22), id(23)),
            Ok(())
        );
        assert_eq!(
            ack.validate_for(open, id(20), id(21), id(22), id(23)),
            Err(Error::InvalidRelease)
        );
    }

    #[test]
    fn hostile_reserved_alias_overflow_and_truncation_refuse() {
        let request = request(GenericFoundingStageV1::FoundAndPermit);
        let mut bytes = request.encode().expect("encode");
        bytes[12] = 1;
        assert_eq!(
            GenericFoundingRequestV1::decode(&bytes),
            Err(Error::NonzeroReserved)
        );
        assert_eq!(
            GenericFoundingRequestV1::decode(&bytes[..399]),
            Err(Error::InvalidLength)
        );
        for offset in
            REQUEST_TAIL_RESERVED_OFFSET..REQUEST_TAIL_RESERVED_OFFSET + REQUEST_TAIL_RESERVED_BYTES
        {
            let mut tail = request.encode().expect("encode");
            *tail.get_mut(offset).expect("reserved byte") = 1;
            assert_eq!(
                GenericFoundingRequestV1::decode(&tail),
                Err(Error::NonzeroReserved),
                "reserved byte {offset} must refuse"
            );
        }
        assert!(
            GenericFoundingRequestV1::new(
                GenericFoundingStageV1::FoundAndPermit,
                3,
                id(1),
                id(2),
                id(3),
                id(3),
                id(5),
                id(6),
                id(7),
                id(8),
                id(9),
                id(10),
                11,
                12,
                13,
                14,
                15,
                16,
                2,
                5,
            )
            .is_err()
        );
        assert!(
            GenericFoundingRequestV1::new(
                GenericFoundingStageV1::FoundAndPermit,
                3,
                id(1),
                id(2),
                id(3),
                id(4),
                id(5),
                id(6),
                id(7),
                id(8),
                id(9),
                id(10),
                11,
                u64::MAX,
                2,
                14,
                15,
                16,
                2,
                5,
            )
            .is_err()
        );
    }

    #[test]
    fn selection_preimage_is_root_free_stage_free_and_coordinate_bound() {
        let found = request(GenericFoundingStageV1::FoundAndPermit);
        let open = request(GenericFoundingStageV1::Open);
        assert_eq!(
            found.selection_preimage().expect("found preimage"),
            open.selection_preimage().expect("open preimage")
        );
        assert_eq!(
            found.selection_config_id().expect("found config"),
            open.selection_config_id().expect("open config")
        );

        let mut rerooted = found;
        rerooted.capability_root = id(0x5a);
        assert_ne!(rerooted, found);
        assert_eq!(
            rerooted.selection_config_id().expect("rerooted config"),
            found.selection_config_id().expect("found config")
        );

        let preimage = found.selection_preimage().expect("preimage");
        assert_eq!(
            preimage
                .get(REQUEST_CAPABILITY_ROOT_OFFSET..REQUEST_CAPABILITY_ROOT_OFFSET + 32)
                .expect("root span"),
            [0; 32].as_slice()
        );
        assert_ne!(preimage, found.encode().expect("encoded request"));

        for mutated in [
            GenericFoundingRequestV1 {
                market: id(0x5b),
                ..found
            },
            GenericFoundingRequestV1 {
                context: id(0x5c),
                ..found
            },
            GenericFoundingRequestV1 {
                hoard: id(0x5d),
                ..found
            },
            GenericFoundingRequestV1 {
                generation: 12,
                ..found
            },
            GenericFoundingRequestV1 {
                quantity: 13,
                ..found
            },
            GenericFoundingRequestV1 {
                expiry_slot: 15,
                ..found
            },
            GenericFoundingRequestV1 {
                projected_resulting_revision: 3,
                ..found
            },
            GenericFoundingRequestV1 {
                capability_entry_index: 6,
                ..found
            },
        ] {
            assert_ne!(
                mutated.selection_config_id().expect("mutated config"),
                found.selection_config_id().expect("found config")
            );
        }
    }

    #[test]
    fn capability_entry_index_occupies_the_former_unchecked_tail() {
        let found = request(GenericFoundingStageV1::FoundAndPermit);
        assert_eq!(found.capability_entry_index(), 5);
        let bytes = found.encode().expect("encode");
        assert_eq!(
            bytes
                .get(REQUEST_ENTRY_INDEX_OFFSET..REQUEST_ENTRY_INDEX_OFFSET + 2)
                .expect("entry span"),
            5_u16.to_le_bytes().as_slice()
        );
        assert_eq!(
            bytes
                .get(
                    REQUEST_TAIL_RESERVED_OFFSET
                        ..REQUEST_TAIL_RESERVED_OFFSET + REQUEST_TAIL_RESERVED_BYTES
                )
                .expect("tail span"),
            [0; REQUEST_TAIL_RESERVED_BYTES].as_slice()
        );
        assert_eq!(GenericFoundingRequestV1::decode(&bytes), Ok(found));
        assert_eq!(
            found
                .with_stage(GenericFoundingStageV1::Open)
                .expect("open")
                .capability_entry_index(),
            5
        );
        // u16::MAX is representable on the wire; the authenticated manifest's
        // actual entry count is the sole bound, so the codec does not guess one.
        let wide = GenericFoundingRequestV1 {
            capability_entry_index: u16::MAX,
            ..found
        };
        let wide_bytes = wide.encode().expect("encode wide");
        assert_eq!(GenericFoundingRequestV1::decode(&wide_bytes), Ok(wide));
    }

    #[test]
    fn funding_list_is_ordered_alias_free_and_width_bounded() {
        let left =
            generic_founding_funding_list_id_v1(&[id(1), id(2), id(3)]).expect("ordered list");
        let right =
            generic_founding_funding_list_id_v1(&[id(3), id(2), id(1)]).expect("reverse list");
        assert_ne!(left, right);
        assert_eq!(
            generic_founding_funding_list_id_v1(&[id(1), id(1)]),
            Err(Error::InvalidCoordinates)
        );
        assert_eq!(
            generic_founding_funding_list_id_v1(&[]),
            Err(Error::InvalidCoordinates)
        );
    }
}
