//! Canonical General V2 Position-transfer wire and structured-custody digest body.

use clutch_structured_claim::DeploymentBinding;

use crate::{
    put, take, Amount, AssetTransferPhasePolicyV1, Error, Result, StructuredClaimActionV1,
    MAX_OUTCOMES, STRUCTURED_CLAIM_FAMILY_TAG, STRUCTURED_CLAIM_FAMILY_VERSION,
};

/// General V2 extension family.
pub const GENERAL_V2_FAMILY_TAG: u8 = 74;
/// General V2 extension family version.
pub const GENERAL_V2_FAMILY_VERSION: u8 = 1;
/// General V2 family-local `TransferPositionAssets` action.
pub const GENERAL_V2_TRANSFER_POSITION_ASSETS_ACTION: u8 = 35;
/// Exact `TransferPositionAssets` payload width.
pub const POSITION_ASSET_TRANSFER_PAYLOAD_BYTES: usize = 298;
/// Domain separated from every persisted identity and generic signer digest.
pub const STRUCTURED_CUSTODY_CALL_V1_DOMAIN: &[u8] =
    b"dragons-clutch/authenticated-structured-custody-call/v1\0";
/// Exact canonical digest-body width, excluding the domain above.
pub const STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES: usize = 1_352;

const IDENTITY_COUNT: usize = 32;
const DEPLOYMENT_SLOT_COUNT: usize = 3;
const HEADER_BYTES: usize = 6;

const _: () = assert!(
    STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES
        == (IDENTITY_COUNT * 32)
            + (DEPLOYMENT_SLOT_COUNT * 8)
            + HEADER_BYTES
            + POSITION_ASSET_TRANSFER_PAYLOAD_BYTES
);

/// Authority semantics for General V2 Position asset transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PositionAssetTransferAuthorityKindV1 {
    /// Account zero is the exact semantic source owner.
    OwnerSigner = 0,
    /// Account zero is a typed wrapper-vault PDA signer and `authority_id` is
    /// the digest of a fully reconstructed ephemeral custody call.
    StructuredCustody = 1,
}

impl PositionAssetTransferAuthorityKindV1 {
    /// Decode the exact one-byte wire value.
    pub const fn from_byte(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::OwnerSigner),
            1 => Ok(Self::StructuredCustody),
            _ => Err(Error::AuthorityUnavailable),
        }
    }

    /// Return the exact one-byte wire value.
    pub const fn to_byte(self) -> u8 {
        self as u8
    }
}

/// Canonical 298-byte General V2 `TransferPositionAssets` body.
///
/// `market` is the full `MarketInstanceV2Id`, not a legacy Market nonce or a
/// mutable runtime address. Wire authority is this codec, never `repr(C)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PositionAssetTransferPayloadV1 {
    /// Full MarketInstanceV2 content identity.
    pub market: [u8; 32],
    /// Exact semantic source Position owner.
    pub source_owner: [u8; 32],
    /// Exact semantic destination Position owner.
    pub destination_owner: [u8; 32],
    /// Exact source Position generation.
    pub source_generation: u64,
    /// Exact destination Position generation.
    pub destination_generation: u64,
    /// Exact source Replay sequence before mutation.
    pub source_replay_sequence: u64,
    /// Exact destination Replay sequence before mutation.
    pub destination_replay_sequence: u64,
    /// Free cash atoms to move.
    pub cash_atoms: Amount,
    /// Free native Eggs to move, zero padded after the Market width.
    pub internal: [Amount; MAX_OUTCOMES],
    /// Exact Active-only or Active-or-Resolved policy.
    pub phase_policy: AssetTransferPhasePolicyV1,
    /// Exact owner or typed structured-custody authority route.
    pub authority_kind: PositionAssetTransferAuthorityKindV1,
    /// Owner identity or domain-separated authenticated custody-call digest.
    pub authority_id: [u8; 32],
}

impl PositionAssetTransferPayloadV1 {
    /// Encode the complete canonical payload accepted by General V2.
    pub fn encode(&self) -> Result<[u8; POSITION_ASSET_TRANSFER_PAYLOAD_BYTES]> {
        self.validate(true)?;
        self.encode_unchecked(self.authority_id)
    }

    /// Encode the authority-neutral payload committed by a custody-call digest.
    ///
    /// The final digest is deliberately replaced by zeroes here so the digest
    /// has no recursive self-reference. All other canonical checks still run.
    pub fn encode_for_authority_digest(
        &self,
    ) -> Result<[u8; POSITION_ASSET_TRANSFER_PAYLOAD_BYTES]> {
        if self.authority_kind != PositionAssetTransferAuthorityKindV1::StructuredCustody {
            return Err(Error::AuthorityUnavailable);
        }
        self.validate(false)?;
        self.encode_unchecked([0; 32])
    }

    /// Return the same transfer with its final typed-custody digest installed.
    pub fn with_custody_authority(self, authority_id: [u8; 32]) -> Result<Self> {
        let value = Self {
            authority_id,
            ..self
        };
        value.validate(true)?;
        if value.authority_kind != PositionAssetTransferAuthorityKindV1::StructuredCustody {
            return Err(Error::AuthorityUnavailable);
        }
        Ok(value)
    }

    fn validate(&self, require_authority: bool) -> Result<()> {
        if self.market == [0; 32]
            || self.source_owner == [0; 32]
            || self.destination_owner == [0; 32]
            || self.source_owner == self.destination_owner
        {
            return Err(Error::InvalidIdentity);
        }
        let mut any = self.cash_atoms != 0;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            any |= self.internal[index] != 0;
            index += 1;
        }
        if !any {
            return Err(Error::ZeroQuantity);
        }
        match self.phase_policy {
            AssetTransferPhasePolicyV1::ActiveOnly
            | AssetTransferPhasePolicyV1::ActiveOrResolved => {}
        }
        match self.authority_kind {
            PositionAssetTransferAuthorityKindV1::OwnerSigner => {
                if self.authority_id != self.source_owner {
                    return Err(Error::AuthorityUnavailable);
                }
            }
            PositionAssetTransferAuthorityKindV1::StructuredCustody => {
                if require_authority && self.authority_id == [0; 32] {
                    return Err(Error::AuthorityUnavailable);
                }
            }
        }
        Ok(())
    }

    fn encode_unchecked(
        &self,
        authority_id: [u8; 32],
    ) -> Result<[u8; POSITION_ASSET_TRANSFER_PAYLOAD_BYTES]> {
        let mut output = [0_u8; POSITION_ASSET_TRANSFER_PAYLOAD_BYTES];
        let mut cursor = 0_usize;
        for identity in [self.market, self.source_owner, self.destination_owner] {
            put(&mut output, &mut cursor, &identity)?;
        }
        for value in [
            self.source_generation,
            self.destination_generation,
            self.source_replay_sequence,
            self.destination_replay_sequence,
            self.cash_atoms,
        ] {
            put(&mut output, &mut cursor, &value.to_le_bytes())?;
        }
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            put(
                &mut output,
                &mut cursor,
                &self.internal[index].to_le_bytes(),
            )?;
            index += 1;
        }
        put(&mut output, &mut cursor, &[self.phase_policy.to_byte()])?;
        put(&mut output, &mut cursor, &[self.authority_kind.to_byte()])?;
        put(&mut output, &mut cursor, &authority_id)?;
        if cursor != POSITION_ASSET_TRANSFER_PAYLOAD_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }
}

/// Decode an exact canonical `TransferPositionAssets` payload.
pub fn decode_position_asset_transfer_payload_v1(
    input: &[u8],
) -> Result<PositionAssetTransferPayloadV1> {
    if input.len() != POSITION_ASSET_TRANSFER_PAYLOAD_BYTES {
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
    let mut index = 0_usize;
    while index < MAX_OUTCOMES {
        internal[index] = read_u64(input, &mut cursor)?;
        index += 1;
    }
    let phase_policy = AssetTransferPhasePolicyV1::from_byte(take(input, &mut cursor, 1)?[0])?;
    let authority_kind =
        PositionAssetTransferAuthorityKindV1::from_byte(take(input, &mut cursor, 1)?[0])?;
    let authority_id = read_key(input, &mut cursor)?;
    if cursor != input.len() {
        return Err(Error::InvalidLength);
    }
    let value = PositionAssetTransferPayloadV1 {
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
        authority_kind,
        authority_id,
    };
    let _ = value.encode()?;
    Ok(value)
}

/// Forgeable pure projection of every fact hashed into typed custody authority.
///
/// This type is not a runtime capability. The wrapper and base adapters must
/// independently reconstruct it from authenticated accounts. Only the adapter's
/// private-field `AuthenticatedStructuredCustodyCallV1` confers authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredCustodyCallProjectionV1 {
    /// Exact callee program executing General V2 action 35.
    pub target_base_program: [u8; 32],
    /// Exact structured-claim family-local action, inferred from Position V3 purposes.
    pub wrapper_local_action: StructuredClaimActionV1,
    /// Immutable 0x88/1 descriptor PDA address.
    pub descriptor_account: [u8; 32],
    /// Domain-separated digest of the exact canonical descriptor body.
    pub descriptor_body_digest: [u8; 32],
    /// Reconstructed canonical native-claim identity.
    pub native_claim_id: [u8; 32],
    /// Reconstructed deployment-bound wrapper-product identity.
    pub wrapper_product_id: [u8; 32],
    /// Exact authenticated wrapper/base/wrapper-token deployments.
    pub deployment: DeploymentBinding,
    /// Canonical base Market account carrying the current lifecycle phase.
    pub market_account: [u8; 32],
    /// Domain-separated digest of the complete canonical Market prestate.
    pub market_body_digest: [u8; 32],
    /// Immutable General V2 MarketBinding PDA.
    pub market_binding_account: [u8; 32],
    /// Domain-separated digest of the exact canonical MarketBinding body.
    pub market_binding_body_digest: [u8; 32],
    /// Base-owned NativeClaimBasis artifact PDA.
    pub native_claim_basis_account: [u8; 32],
    /// Typed exact-body NativeClaimBasis identity.
    pub native_claim_basis_id: [u8; 32],
    /// Base-owned MarketInstanceV2 artifact PDA.
    pub market_instance_account: [u8; 32],
    /// Typed MarketInstanceV2 content identity.
    pub market_instance_id: [u8; 32],
    /// Immutable Realm identity selected by the Market.
    pub realm_id: [u8; 32],
    /// Immutable Realm-selected collateral-policy content identity.
    pub collateral_policy_id: [u8; 32],
    /// Exact compiled collateral parser/CPI release content identity.
    pub collateral_release_id: [u8; 32],
    /// Actual wrapper vault-owner PDA and CPI signer.
    pub vault_authority: [u8; 32],
    /// Actual user Position controller and outer transaction signer.
    pub user_actor: [u8; 32],
    /// Exact source Position V3 PDA.
    pub source_position_account: [u8; 32],
    /// Canonical semantic digest of the complete source Position V3 prestate.
    pub source_position_body_digest: [u8; 32],
    /// Exact source current-generation Replay-successor PDA.
    pub source_replay_account: [u8; 32],
    /// Domain-separated digest of its complete canonical prestate.
    pub source_replay_body_digest: [u8; 32],
    /// Exact destination Position V3 PDA.
    pub destination_position_account: [u8; 32],
    /// Canonical semantic digest of the complete destination Position V3 prestate.
    pub destination_position_body_digest: [u8; 32],
    /// Exact destination current-generation Replay-successor PDA.
    pub destination_replay_account: [u8; 32],
    /// Domain-separated digest of its complete canonical prestate.
    pub destination_replay_body_digest: [u8; 32],
    /// Exact final General V2 transfer payload; its authority digest is zeroed
    /// only while constructing this projection's own preimage.
    pub transfer: PositionAssetTransferPayloadV1,
}

impl StructuredCustodyCallProjectionV1 {
    /// Encode the exact digest body reconstructed independently on both sides of CPI.
    pub fn encode_preimage(&self) -> Result<[u8; STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES]> {
        self.validate()?;
        let mut output = [0_u8; STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES];
        let mut cursor = 0_usize;
        put(&mut output, &mut cursor, &self.target_base_program)?;
        put(
            &mut output,
            &mut cursor,
            &[
                GENERAL_V2_FAMILY_TAG,
                GENERAL_V2_FAMILY_VERSION,
                GENERAL_V2_TRANSFER_POSITION_ASSETS_ACTION,
                STRUCTURED_CLAIM_FAMILY_TAG,
                STRUCTURED_CLAIM_FAMILY_VERSION,
                self.wrapper_local_action.tag(),
            ],
        )?;
        for identity in [
            self.descriptor_account,
            self.descriptor_body_digest,
            self.native_claim_id,
            self.wrapper_product_id,
            self.deployment.wrapper_program,
            self.deployment.wrapper_program_data,
        ] {
            put(&mut output, &mut cursor, &identity)?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.deployment.wrapper_deployment_slot.to_le_bytes(),
        )?;
        for identity in [
            self.deployment.base_program,
            self.deployment.base_program_data,
        ] {
            put(&mut output, &mut cursor, &identity)?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.deployment.base_deployment_slot.to_le_bytes(),
        )?;
        for identity in [
            self.deployment.token_2022_program,
            self.deployment.token_2022_program_data,
        ] {
            put(&mut output, &mut cursor, &identity)?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.deployment.token_2022_deployment_slot.to_le_bytes(),
        )?;
        for identity in [
            self.market_account,
            self.market_body_digest,
            self.market_binding_account,
            self.market_binding_body_digest,
            self.native_claim_basis_account,
            self.native_claim_basis_id,
            self.market_instance_account,
            self.market_instance_id,
            self.realm_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.vault_authority,
            self.user_actor,
            self.source_position_account,
            self.source_position_body_digest,
            self.source_replay_account,
            self.source_replay_body_digest,
            self.destination_position_account,
            self.destination_position_body_digest,
            self.destination_replay_account,
            self.destination_replay_body_digest,
        ] {
            put(&mut output, &mut cursor, &identity)?;
        }
        put(
            &mut output,
            &mut cursor,
            &self.transfer.encode_for_authority_digest()?,
        )?;
        if cursor != STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    fn validate(&self) -> Result<()> {
        self.deployment
            .validate()
            .map_err(|_| Error::AuthorityUnavailable)?;
        if self.target_base_program != self.deployment.base_program
            || self.market_instance_id != self.transfer.market
            || self.wrapper_local_action != StructuredClaimActionV1::WrapCanonical
                && self.wrapper_local_action != StructuredClaimActionV1::UnwrapCanonical
            || self.transfer.authority_kind
                != PositionAssetTransferAuthorityKindV1::StructuredCustody
        {
            return Err(Error::AuthorityUnavailable);
        }
        let identities = [
            self.descriptor_account,
            self.descriptor_body_digest,
            self.native_claim_id,
            self.wrapper_product_id,
            self.market_account,
            self.market_body_digest,
            self.market_binding_account,
            self.market_binding_body_digest,
            self.native_claim_basis_account,
            self.native_claim_basis_id,
            self.market_instance_account,
            self.market_instance_id,
            self.realm_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.vault_authority,
            self.user_actor,
            self.source_position_account,
            self.source_position_body_digest,
            self.source_replay_account,
            self.source_replay_body_digest,
            self.destination_position_account,
            self.destination_position_body_digest,
            self.destination_replay_account,
            self.destination_replay_body_digest,
        ];
        let mut index = 0_usize;
        while index < identities.len() {
            if identities[index] == [0; 32] {
                return Err(Error::InvalidIdentity);
            }
            index += 1;
        }
        if self.source_position_account == self.destination_position_account
            || self.source_replay_account == self.destination_replay_account
            || self.vault_authority == self.user_actor
        {
            return Err(Error::InvalidIdentity);
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use clutch_structured_claim::DeploymentBinding;

    use super::*;

    fn payload(authority_id: [u8; 32]) -> PositionAssetTransferPayloadV1 {
        let mut internal = [0_u64; MAX_OUTCOMES];
        internal[3] = 17;
        PositionAssetTransferPayloadV1 {
            market: [1; 32],
            source_owner: [2; 32],
            destination_owner: [3; 32],
            source_generation: 4,
            destination_generation: 5,
            source_replay_sequence: 6,
            destination_replay_sequence: 7,
            cash_atoms: 8,
            internal,
            phase_policy: AssetTransferPhasePolicyV1::ActiveOnly,
            authority_kind: PositionAssetTransferAuthorityKindV1::StructuredCustody,
            authority_id,
        }
    }

    #[test]
    fn transfer_codec_is_exact_and_authority_neutralization_is_nonrecursive() {
        let value = payload([9; 32]);
        let bytes = value.encode().unwrap();
        assert_eq!(bytes.len(), POSITION_ASSET_TRANSFER_PAYLOAD_BYTES);
        assert_eq!(decode_position_asset_transfer_payload_v1(&bytes), Ok(value));
        assert_eq!(&bytes[266..], &[9; 32]);
        let neutral = value.encode_for_authority_digest().unwrap();
        assert_eq!(&neutral[..266], &bytes[..266]);
        assert_eq!(&neutral[266..], &[0; 32]);
    }

    #[test]
    fn owner_authority_cannot_name_an_unrelated_signer() {
        let mut value = payload([9; 32]);
        value.authority_kind = PositionAssetTransferAuthorityKindV1::OwnerSigner;
        assert_eq!(value.encode(), Err(Error::AuthorityUnavailable));
        value.authority_id = value.source_owner;
        assert!(value.encode().is_ok());
    }

    #[test]
    fn custody_projection_binds_headers_and_zeroes_only_the_recursive_digest() {
        let projection = StructuredCustodyCallProjectionV1 {
            target_base_program: [7; 32],
            wrapper_local_action: StructuredClaimActionV1::WrapCanonical,
            descriptor_account: [10; 32],
            descriptor_body_digest: [11; 32],
            native_claim_id: [12; 32],
            wrapper_product_id: [13; 32],
            deployment: DeploymentBinding {
                wrapper_program: [4; 32],
                wrapper_program_data: [5; 32],
                wrapper_deployment_slot: 1,
                base_program: [7; 32],
                base_program_data: [8; 32],
                base_deployment_slot: 2,
                token_2022_program: [14; 32],
                token_2022_program_data: [15; 32],
                token_2022_deployment_slot: 3,
            },
            market_account: [34; 32],
            market_body_digest: [35; 32],
            market_binding_account: [16; 32],
            market_binding_body_digest: [17; 32],
            native_claim_basis_account: [18; 32],
            native_claim_basis_id: [19; 32],
            market_instance_account: [20; 32],
            market_instance_id: [1; 32],
            realm_id: [21; 32],
            collateral_policy_id: [22; 32],
            collateral_release_id: [23; 32],
            vault_authority: [24; 32],
            user_actor: [25; 32],
            source_position_account: [26; 32],
            source_position_body_digest: [27; 32],
            source_replay_account: [28; 32],
            source_replay_body_digest: [29; 32],
            destination_position_account: [30; 32],
            destination_position_body_digest: [31; 32],
            destination_replay_account: [32; 32],
            destination_replay_body_digest: [33; 32],
            transfer: payload([9; 32]),
        };
        let bytes = projection.encode_preimage().unwrap();
        assert_eq!(bytes.len(), STRUCTURED_CUSTODY_CALL_PREIMAGE_BYTES);
        assert_eq!(&bytes[..32], &[7; 32]);
        assert_eq!(
            &bytes[32..38],
            &[
                74,
                1,
                35,
                75,
                1,
                StructuredClaimActionV1::WrapCanonical.tag()
            ]
        );
        assert_eq!(&bytes[bytes.len() - 32..], &[0; 32]);
    }
}
