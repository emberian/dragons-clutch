//! Strict family-local instruction payload contract.

use crate::{
    put, take, Amount, AssetTransferAuthorityKindV1, AssetTransferPhasePolicyV1,
    AtomicPositionAssetTransferRequestV1, Error, Result, MAX_OUTCOMES,
};

/// Reserved structured-claim extension family.
pub const STRUCTURED_CLAIM_FAMILY_TAG: u8 = 75;
/// Structured-claim extension family version.
pub const STRUCTURED_CLAIM_FAMILY_VERSION: u8 = 1;
/// Exact CreateDescriptor payload width.
pub const CREATE_DESCRIPTOR_PAYLOAD_BYTES: usize = 32 + 32 + (MAX_OUTCOMES * 8);
/// Exact supply-sensitive wrapper mutation payload width.
pub const WRAPPER_QUANTITY_PAYLOAD_BYTES: usize = 32 + (5 * 8);
/// Exact vault-only mutation payload width.
pub const VAULT_MUTATION_PAYLOAD_BYTES: usize = 32 + (2 * 8);
/// Exact General V2 action-35 Position asset-transfer payload width.
pub const GENERAL_POSITION_ASSET_TRANSFER_PAYLOAD_BYTES: usize =
    (3 * 32) + (5 * 8) + (MAX_OUTCOMES * 8) + 1 + 1 + 32;
const _: () = assert!(GENERAL_POSITION_ASSET_TRANSFER_PAYLOAD_BYTES == 298);

/// Canonical General V2 action-35 payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralPositionAssetTransferPayloadV1 {
    /// Supply- and Hoard-neutral transfer request owned by the pure planner.
    pub transfer: AtomicPositionAssetTransferRequestV1,
    /// Owner signature or future typed custody-capability authorization.
    pub authority_kind: AssetTransferAuthorityKindV1,
    /// Exact signing owner or custody-capability identity.
    pub authority_id: [u8; 32],
}

impl GeneralPositionAssetTransferPayloadV1 {
    /// Encode exactly 298 canonical bytes without relying on `repr` width.
    pub fn encode(&self) -> Result<[u8; GENERAL_POSITION_ASSET_TRANSFER_PAYLOAD_BYTES]> {
        validate_general_position_transfer_payload_v1(self)?;
        let mut output = [0_u8; GENERAL_POSITION_ASSET_TRANSFER_PAYLOAD_BYTES];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &self.transfer.market)?;
        put(&mut output, &mut cursor, &self.transfer.source_owner)?;
        put(&mut output, &mut cursor, &self.transfer.destination_owner)?;
        for value in [
            self.transfer.source_generation,
            self.transfer.destination_generation,
            self.transfer.source_replay_sequence,
            self.transfer.destination_replay_sequence,
            self.transfer.cash_atoms,
        ] {
            put(&mut output, &mut cursor, &value.to_le_bytes())?;
        }
        for value in self.transfer.internal {
            put(&mut output, &mut cursor, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            &mut cursor,
            &[self.transfer.phase_policy.to_byte()],
        )?;
        put(&mut output, &mut cursor, &[self.authority_kind.to_byte()])?;
        put(&mut output, &mut cursor, &self.authority_id)?;
        if cursor != GENERAL_POSITION_ASSET_TRANSFER_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    /// Decode exactly 298 hostile bytes and re-run canonical structural checks.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != GENERAL_POSITION_ASSET_TRANSFER_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        let mut cursor = 0_usize;
        let market = read_key(input, &mut cursor)?;
        let source_owner = read_key(input, &mut cursor)?;
        let destination_owner = read_key(input, &mut cursor)?;
        let source_generation = read_u64(input, &mut cursor)?;
        let destination_generation = read_u64(input, &mut cursor)?;
        let source_replay_sequence = read_u64(input, &mut cursor)?;
        let destination_replay_sequence = read_u64(input, &mut cursor)?;
        let cash_atoms = read_u64(input, &mut cursor)?;
        let mut internal = [0_u64; MAX_OUTCOMES];
        let mut index = 0usize;
        while index < MAX_OUTCOMES {
            internal[index] = read_u64(input, &mut cursor)?;
            index += 1;
        }
        let phase_policy = AssetTransferPhasePolicyV1::from_byte(read_u8(input, &mut cursor)?)?;
        let authority_kind = AssetTransferAuthorityKindV1::from_byte(read_u8(input, &mut cursor)?)?;
        let authority_id = read_key(input, &mut cursor)?;
        if cursor != GENERAL_POSITION_ASSET_TRANSFER_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        let value = Self {
            transfer: AtomicPositionAssetTransferRequestV1 {
                market,
                source_owner,
                destination_owner,
                source_generation,
                destination_generation,
                source_replay_sequence,
                destination_replay_sequence,
                cash_atoms,
                internal,
                phase_policy,
            },
            authority_kind,
            authority_id,
        };
        validate_general_position_transfer_payload_v1(&value)?;
        Ok(value)
    }
}

fn validate_general_position_transfer_payload_v1(
    payload: &GeneralPositionAssetTransferPayloadV1,
) -> Result<()> {
    if payload.transfer.market == [0; 32]
        || payload.transfer.source_owner == [0; 32]
        || payload.transfer.destination_owner == [0; 32]
        || payload.transfer.source_owner == payload.transfer.destination_owner
        || payload.authority_id == [0; 32]
    {
        return Err(Error::InvalidIdentity);
    }
    if payload.authority_kind == AssetTransferAuthorityKindV1::OwnerSigner
        && payload.authority_id != payload.transfer.source_owner
    {
        return Err(Error::InvalidIdentity);
    }
    let mut any = payload.transfer.cash_atoms != 0;
    for quantity in payload.transfer.internal {
        any |= quantity != 0;
    }
    if !any {
        return Err(Error::ZeroQuantity);
    }
    Ok(())
}

/// Family-local structured-claim action allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StructuredClaimActionV1 {
    /// Create the immutable descriptor, mint, and empty vault Position.
    CreateDescriptor = 1,
    /// Mint wrappers from canonical cash-plus-residual backing.
    WrapCanonical = 2,
    /// Mint wrappers from a full native-Egg vector and compress complete sets.
    WrapFull = 3,
    /// Burn wrappers and return canonical cash-plus-residual backing.
    UnwrapCanonical = 4,
    /// Burn wrappers and expand backing into a full native-Egg vector.
    UnwrapFull = 5,
    /// Move direct-burn surplus to beneficiary-free base donations.
    CompactDonation = 6,
    /// Burn an exact terminal lot and redeem its aggregate native value.
    RedeemTerminal = 7,
    /// Permanently retire a zero-supply, zero-backing descriptor tombstone.
    Retire = 8,
}

impl StructuredClaimActionV1 {
    /// First allocated family-local action.
    pub const FIRST_TAG: u8 = 1;
    /// Last allocated family-local action.
    pub const LAST_TAG: u8 = 8;

    /// Decode one exact family-local action.
    pub const fn from_tag(tag: u8) -> Result<Self> {
        match tag {
            1 => Ok(Self::CreateDescriptor),
            2 => Ok(Self::WrapCanonical),
            3 => Ok(Self::WrapFull),
            4 => Ok(Self::UnwrapCanonical),
            5 => Ok(Self::UnwrapFull),
            6 => Ok(Self::CompactDonation),
            7 => Ok(Self::RedeemTerminal),
            8 => Ok(Self::Retire),
            _ => Err(Error::UnknownAction),
        }
    }

    /// Return the exact family-local wire byte.
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

/// Content-addressed descriptor construction payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct CreateDescriptorPayloadV1 {
    /// SHA-256 of the reconstructed native-claim preimage.
    pub native_claim_id: [u8; 32],
    /// SHA-256 of the deployment-bound wrapper-product preimage.
    pub wrapper_product_id: [u8; 32],
    /// Primitive GCD-one native-Egg coefficients.
    pub primitive: [Amount; MAX_OUTCOMES],
}

impl CreateDescriptorPayloadV1 {
    /// Encode the exact payload body.
    pub fn encode(&self) -> Result<[u8; CREATE_DESCRIPTOR_PAYLOAD_BYTES]> {
        if self.native_claim_id == [0; 32] || self.wrapper_product_id == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        let mut output = [0_u8; CREATE_DESCRIPTOR_PAYLOAD_BYTES];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &self.native_claim_id)?;
        put(&mut output, &mut cursor, &self.wrapper_product_id)?;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            put(
                &mut output,
                &mut cursor,
                &self.primitive[index].to_le_bytes(),
            )?;
            index += 1;
        }
        if cursor != CREATE_DESCRIPTOR_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != CREATE_DESCRIPTOR_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        let mut cursor = 0_usize;
        let native_claim_id = read_key(input, &mut cursor)?;
        let wrapper_product_id = read_key(input, &mut cursor)?;
        let mut primitive = [0_u64; MAX_OUTCOMES];
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            primitive[index] = read_u64(input, &mut cursor)?;
            index += 1;
        }
        let value = Self {
            native_claim_id,
            wrapper_product_id,
            primitive,
        };
        let _ = value.encode()?;
        Ok(value)
    }
}

/// Quantity-bearing wrap, unwind, or terminal-redemption payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct WrapperQuantityPayloadV1 {
    /// Exact wrapper product identity.
    pub wrapper_product_id: [u8; 32],
    /// Wrapper atoms to mint, burn, or redeem.
    pub quantity: Amount,
    /// Exact user Position generation.
    pub user_generation: u64,
    /// Exact user Replay sequence.
    pub user_replay_sequence: u64,
    /// Exact wrapper-vault Position generation.
    pub vault_generation: u64,
    /// Exact wrapper-vault Replay sequence.
    pub vault_replay_sequence: u64,
}

impl WrapperQuantityPayloadV1 {
    /// Encode the exact payload body.
    pub fn encode(&self) -> Result<[u8; WRAPPER_QUANTITY_PAYLOAD_BYTES]> {
        if self.wrapper_product_id == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        if self.quantity == 0 {
            return Err(Error::ZeroQuantity);
        }
        let mut output = [0_u8; WRAPPER_QUANTITY_PAYLOAD_BYTES];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &self.wrapper_product_id)?;
        put(&mut output, &mut cursor, &self.quantity.to_le_bytes())?;
        put(
            &mut output,
            &mut cursor,
            &self.user_generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.user_replay_sequence.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.vault_generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.vault_replay_sequence.to_le_bytes(),
        )?;
        if cursor != WRAPPER_QUANTITY_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != WRAPPER_QUANTITY_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        let mut cursor = 0_usize;
        let value = Self {
            wrapper_product_id: read_key(input, &mut cursor)?,
            quantity: read_u64(input, &mut cursor)?,
            user_generation: read_u64(input, &mut cursor)?,
            user_replay_sequence: read_u64(input, &mut cursor)?,
            vault_generation: read_u64(input, &mut cursor)?,
            vault_replay_sequence: read_u64(input, &mut cursor)?,
        };
        let _ = value.encode()?;
        Ok(value)
    }
}

/// Vault-only donation/retirement payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct VaultMutationPayloadV1 {
    /// Exact wrapper product identity.
    pub wrapper_product_id: [u8; 32],
    /// Exact wrapper-vault Position generation.
    pub vault_generation: u64,
    /// Exact wrapper-vault Replay sequence.
    pub vault_replay_sequence: u64,
}

impl VaultMutationPayloadV1 {
    /// Encode the exact payload body.
    pub fn encode(&self) -> Result<[u8; VAULT_MUTATION_PAYLOAD_BYTES]> {
        if self.wrapper_product_id == [0; 32] {
            return Err(Error::InvalidIdentity);
        }
        let mut output = [0_u8; VAULT_MUTATION_PAYLOAD_BYTES];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &self.wrapper_product_id)?;
        put(
            &mut output,
            &mut cursor,
            &self.vault_generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            &mut cursor,
            &self.vault_replay_sequence.to_le_bytes(),
        )?;
        if cursor != VAULT_MUTATION_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != VAULT_MUTATION_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        let mut cursor = 0_usize;
        let value = Self {
            wrapper_product_id: read_key(input, &mut cursor)?,
            vault_generation: read_u64(input, &mut cursor)?,
            vault_replay_sequence: read_u64(input, &mut cursor)?,
        };
        let _ = value.encode()?;
        Ok(value)
    }
}

/// Strictly decoded action-specific payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredClaimPayloadV1 {
    /// Descriptor construction.
    CreateDescriptor(CreateDescriptorPayloadV1),
    /// Canonical backing wrap.
    WrapCanonical(WrapperQuantityPayloadV1),
    /// Full-vector wrap.
    WrapFull(WrapperQuantityPayloadV1),
    /// Canonical backing unwind.
    UnwrapCanonical(WrapperQuantityPayloadV1),
    /// Full-vector unwind.
    UnwrapFull(WrapperQuantityPayloadV1),
    /// Beneficiary-free surplus compaction.
    CompactDonation(VaultMutationPayloadV1),
    /// Exact terminal aggregate redemption.
    RedeemTerminal(WrapperQuantityPayloadV1),
    /// Permanent descriptor retirement.
    Retire(VaultMutationPayloadV1),
}

/// Decode only the exact payload width belonging to one allocated action.
pub fn decode_structured_claim_payload_v1(
    action_tag: u8,
    input: &[u8],
) -> Result<StructuredClaimPayloadV1> {
    match StructuredClaimActionV1::from_tag(action_tag)? {
        StructuredClaimActionV1::CreateDescriptor => Ok(
            StructuredClaimPayloadV1::CreateDescriptor(CreateDescriptorPayloadV1::decode(input)?),
        ),
        StructuredClaimActionV1::WrapCanonical => Ok(StructuredClaimPayloadV1::WrapCanonical(
            WrapperQuantityPayloadV1::decode(input)?,
        )),
        StructuredClaimActionV1::WrapFull => Ok(StructuredClaimPayloadV1::WrapFull(
            WrapperQuantityPayloadV1::decode(input)?,
        )),
        StructuredClaimActionV1::UnwrapCanonical => Ok(StructuredClaimPayloadV1::UnwrapCanonical(
            WrapperQuantityPayloadV1::decode(input)?,
        )),
        StructuredClaimActionV1::UnwrapFull => Ok(StructuredClaimPayloadV1::UnwrapFull(
            WrapperQuantityPayloadV1::decode(input)?,
        )),
        StructuredClaimActionV1::CompactDonation => Ok(StructuredClaimPayloadV1::CompactDonation(
            VaultMutationPayloadV1::decode(input)?,
        )),
        StructuredClaimActionV1::RedeemTerminal => Ok(StructuredClaimPayloadV1::RedeemTerminal(
            WrapperQuantityPayloadV1::decode(input)?,
        )),
        StructuredClaimActionV1::Retire => Ok(StructuredClaimPayloadV1::Retire(
            VaultMutationPayloadV1::decode(input)?,
        )),
    }
}

fn read_key(input: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    let mut value = [0_u8; 32];
    value.copy_from_slice(take(input, cursor, 32)?);
    Ok(value)
}

fn read_u64(input: &[u8], cursor: &mut usize) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(take(input, cursor, 8)?);
    Ok(u64::from_le_bytes(bytes))
}

fn read_u8(input: &[u8], cursor: &mut usize) -> Result<u8> {
    Ok(*take(input, cursor, 1)?
        .first()
        .ok_or(Error::InvalidLength)?)
}
