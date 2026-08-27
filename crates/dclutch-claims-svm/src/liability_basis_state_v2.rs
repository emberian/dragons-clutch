//! Canonical fixed-header, runtime-width LiabilityBasisV2 Claims state.
//!
//! This module is the sole SDK-free owner of the aggregate and Position byte
//! layouts shared by the Claims adapter and unsigned operators. It allocates
//! nothing: callers provide exact output slices and borrowed claim vectors.

use core::convert::TryInto;

/// LiabilityBasisV2 aggregate header bytes before `u64[claim_count]` supplies.
pub const LIABILITY_BASIS_MARKET_HEADER_BYTES_V2: usize = 256;
/// LiabilityBasisV2 Position header bytes before `u64[claim_count]` balances.
pub const LIABILITY_BASIS_POSITION_HEADER_BYTES_V2: usize = 128;
/// Canonical Claims aggregate PDA seed domain.
pub const LIABILITY_BASIS_MARKET_SEED_V2: &[u8] = b"dclutch:lbv2:market";
/// Canonical aggregate magic.
pub const LIABILITY_BASIS_MARKET_MAGIC_V2: [u8; 8] = *b"DCLLBM02";
/// Canonical Position magic.
pub const LIABILITY_BASIS_POSITION_MAGIC_V2: [u8; 8] = *b"DCLLBP02";
/// Implemented state ABI version.
pub const LIABILITY_BASIS_STATE_VERSION_V2: u16 = 2;

const MARKET_CLAIM_COUNT_OFFSET: usize = 12;
const MARKET_REVISION_OFFSET: usize = 16;
const MARKET_LOGICAL_ID_OFFSET: usize = 24;
const MARKET_RELEASE_SET_OFFSET: usize = 56;
const MARKET_REGISTRY_OFFSET: usize = 88;
const MARKET_PRODUCT_OFFSET: usize = 120;
const MARKET_BASIS_OFFSET: usize = 152;
const MARKET_REALM_OFFSET: usize = 184;
const MARKET_CUSTODY_CONTEXT_OFFSET: usize = 216;
const MARKET_GENERATION_OFFSET: usize = 248;

const POSITION_CLAIM_COUNT_OFFSET: usize = 12;
const POSITION_REVISION_OFFSET: usize = 16;
const POSITION_MARKET_OFFSET: usize = 24;
const POSITION_OWNER_OFFSET: usize = 56;
const POSITION_BASIS_OFFSET: usize = 88;
const POSITION_RESERVED_OFFSET: usize = 120;

/// Canonical patch/projection coordinates of a LiabilityBasisV2 aggregate.
///
/// Account-profile generators consume these typed coordinates instead of
/// restating private wire offsets. The hostile decoder remains the authority
/// for admitting account bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisMarketLayoutV2;

impl LiabilityBasisMarketLayoutV2 {
    /// Runtime claim count (`u32`, little-endian).
    pub const CLAIM_COUNT: usize = MARKET_CLAIM_COUNT_OFFSET;
    /// Aggregate optimistic revision (`u64`, little-endian).
    pub const REVISION: usize = MARKET_REVISION_OFFSET;
    /// Canonical logical Core Market identity.
    pub const LOGICAL_MARKET: usize = MARKET_LOGICAL_ID_OFFSET;
    /// Immutable selected release set.
    pub const RELEASE_SET: usize = MARKET_RELEASE_SET_OFFSET;
    /// Immutable selected Registry program.
    pub const REGISTRY_PROGRAM: usize = MARKET_REGISTRY_OFFSET;
    /// Finalized Product-instance content identity.
    pub const PRODUCT_INSTANCE: usize = MARKET_PRODUCT_OFFSET;
    /// Semantic LiabilityBasis identity.
    pub const BASIS: usize = MARKET_BASIS_OFFSET;
    /// Immutable Realm content identity.
    pub const REALM: usize = MARKET_REALM_OFFSET;
    /// The Market's Custody namespace: replay AND every Vault compartment.
    pub const CUSTODY_CONTEXT: usize = MARKET_CUSTODY_CONTEXT_OFFSET;
    /// Immutable Market generation.
    pub const GENERATION: usize = MARKET_GENERATION_OFFSET;
    /// Runtime supply vector base; each entry is one little-endian `u64`.
    pub const SUPPLIES: usize = LIABILITY_BASIS_MARKET_HEADER_BYTES_V2;
    /// Runtime supply-vector element stride.
    pub const SUPPLY_STRIDE: usize = 8;

    /// Hostile-decode and atomically copy the aggregate revision.
    pub fn copy_revision_into(input: &[u8], output: &mut u64) -> Result<()> {
        let candidate = LiabilityBasisMarketViewV2::decode(input)?.revision;
        *output = candidate;
        Ok(())
    }
}

/// Canonical patch/projection coordinates of a LiabilityBasisV2 Position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisPositionLayoutV2;

impl LiabilityBasisPositionLayoutV2 {
    /// Runtime claim count (`u32`, little-endian).
    pub const CLAIM_COUNT: usize = POSITION_CLAIM_COUNT_OFFSET;
    /// Position optimistic revision (`u64`, little-endian).
    pub const REVISION: usize = POSITION_REVISION_OFFSET;
    /// Claims aggregate account identity.
    pub const MARKET: usize = POSITION_MARKET_OFFSET;
    /// Sole Position owner.
    pub const OWNER: usize = POSITION_OWNER_OFFSET;
    /// Semantic LiabilityBasis identity.
    pub const BASIS: usize = POSITION_BASIS_OFFSET;
    /// Runtime balance vector base; each entry is one little-endian `u64`.
    pub const BALANCES: usize = LIABILITY_BASIS_POSITION_HEADER_BYTES_V2;
    /// Runtime balance-vector element stride.
    pub const BALANCE_STRIDE: usize = 8;

    /// Hostile-decode and atomically copy the Position revision.
    pub fn copy_revision_into(input: &[u8], output: &mut u64) -> Result<()> {
        let candidate = LiabilityBasisPositionViewV2::decode(input)?.revision;
        *output = candidate;
        Ok(())
    }
}

/// Stable hostile-decode or encoding refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiabilityBasisStateErrorV2 {
    /// Account bytes did not have the exact count-derived width.
    InvalidLength,
    /// Magic selected another state family.
    InvalidMagic,
    /// State version was unsupported.
    UnsupportedVersion,
    /// Reserved bytes were nonzero.
    NonCanonical,
    /// A persisted identity was zero.
    ZeroIdentity,
    /// The runtime claim count was zero or overflowed address arithmetic.
    InvalidClaimCount,
    /// A requested claim index was outside the runtime width.
    InvalidClaimIndex,
}

/// Result alias for LiabilityBasisV2 state operations.
pub type Result<T> = core::result::Result<T, LiabilityBasisStateErrorV2>;

/// Immutable aggregate construction input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisMarketInputV2 {
    /// Claims aggregate revision.
    pub revision: u64,
    /// Canonical Core Market PDA.
    pub logical_market: [u8; 32],
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Immutable selected Registry program.
    pub registry_program: [u8; 32],
    /// Finalized Product-instance digest.
    pub product_instance_id: [u8; 32],
    /// Semantic LiabilityBasisV2 identity.
    pub basis_id: [u8; 32],
    /// Immutable Realm digest.
    pub realm_id: [u8; 32],
    /// The Market's Custody namespace, and the sole persisted owner of it.
    ///
    /// One `context` coordinate, used for BOTH the Custody replay
    /// (`[replay-domain, market, release_set, context]`) and every Vault of
    /// this Market (`[vault-domain, market, release_set, context,
    /// compartment]`). `FoundingV5` writes the value it authenticated against
    /// the Core-owned permit; no consumer may re-guess it, and in particular it
    /// is NOT the Market address — see
    /// `docs/decisions/0008-custody-namespace-owner.md`.
    pub custody_context: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
}

/// Immutable Position construction input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisPositionInputV2 {
    /// Position revision.
    pub revision: u64,
    /// Claims aggregate account identity.
    pub market_account: [u8; 32],
    /// Sole Position owner.
    pub owner: [u8; 32],
    /// Semantic LiabilityBasisV2 identity.
    pub basis_id: [u8; 32],
}

/// Hostile-decoded aggregate header joined to its exact runtime tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisMarketViewV2 {
    /// Runtime claim count.
    pub claim_count: u32,
    /// Aggregate revision.
    pub revision: u64,
    /// Canonical Core Market PDA.
    pub logical_market: [u8; 32],
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Immutable selected Registry program.
    pub registry_program: [u8; 32],
    /// Finalized Product-instance digest.
    pub product_instance_id: [u8; 32],
    /// Semantic LiabilityBasisV2 identity.
    pub basis_id: [u8; 32],
    /// Immutable Realm digest.
    pub realm_id: [u8; 32],
    /// The Market's Custody namespace, and the sole persisted owner of it.
    ///
    /// One `context` coordinate, used for BOTH the Custody replay
    /// (`[replay-domain, market, release_set, context]`) and every Vault of
    /// this Market (`[vault-domain, market, release_set, context,
    /// compartment]`). `FoundingV5` writes the value it authenticated against
    /// the Core-owned permit; no consumer may re-guess it, and in particular it
    /// is NOT the Market address — see
    /// `docs/decisions/0008-custody-namespace-owner.md`.
    pub custody_context: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
}

impl LiabilityBasisMarketViewV2 {
    /// Decode one exact aggregate and validate every fixed identity and tail byte.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_prefix(
            bytes,
            LIABILITY_BASIS_MARKET_MAGIC_V2,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
        )?;
        require_zero(bytes, 10, 2)?;
        let value = Self {
            claim_count: read_u32(bytes, MARKET_CLAIM_COUNT_OFFSET)?,
            revision: read_u64(bytes, MARKET_REVISION_OFFSET)?,
            logical_market: read_array(bytes, MARKET_LOGICAL_ID_OFFSET)?,
            release_set: read_array(bytes, MARKET_RELEASE_SET_OFFSET)?,
            registry_program: read_array(bytes, MARKET_REGISTRY_OFFSET)?,
            product_instance_id: read_array(bytes, MARKET_PRODUCT_OFFSET)?,
            basis_id: read_array(bytes, MARKET_BASIS_OFFSET)?,
            realm_id: read_array(bytes, MARKET_REALM_OFFSET)?,
            custody_context: read_array(bytes, MARKET_CUSTODY_CONTEXT_OFFSET)?,
            generation: read_u64(bytes, MARKET_GENERATION_OFFSET)?,
        };
        require_nonzero(&[
            value.logical_market,
            value.release_set,
            value.registry_program,
            value.product_instance_id,
            value.basis_id,
            value.realm_id,
            value.custody_context,
        ])?;
        if value.claim_count == 0
            || bytes.len()
                != liability_basis_vector_width_v2(
                    LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
                    value.claim_count,
                )?
        {
            return Err(LiabilityBasisStateErrorV2::InvalidLength);
        }
        Ok(value)
    }

    /// Read one exact supply atom from the runtime tail.
    pub fn supply(self, bytes: &[u8], claim_index: u32) -> Result<u64> {
        if Self::decode(bytes)? != self {
            return Err(LiabilityBasisStateErrorV2::NonCanonical);
        }
        read_claim_v2(
            bytes,
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            self.claim_count,
            claim_index,
        )
    }
}

/// Hostile-decoded Position header joined to its exact runtime tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiabilityBasisPositionViewV2 {
    /// Runtime claim count.
    pub claim_count: u32,
    /// Position revision.
    pub revision: u64,
    /// Claims aggregate account identity.
    pub market_account: [u8; 32],
    /// Sole Position owner.
    pub owner: [u8; 32],
    /// Semantic LiabilityBasisV2 identity.
    pub basis_id: [u8; 32],
}

impl LiabilityBasisPositionViewV2 {
    /// Decode one exact Position and validate every fixed identity and tail byte.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        require_prefix(
            bytes,
            LIABILITY_BASIS_POSITION_MAGIC_V2,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        )?;
        require_zero(bytes, 10, 2)?;
        require_zero(bytes, POSITION_RESERVED_OFFSET, 8)?;
        let value = Self {
            claim_count: read_u32(bytes, POSITION_CLAIM_COUNT_OFFSET)?,
            revision: read_u64(bytes, POSITION_REVISION_OFFSET)?,
            market_account: read_array(bytes, POSITION_MARKET_OFFSET)?,
            owner: read_array(bytes, POSITION_OWNER_OFFSET)?,
            basis_id: read_array(bytes, POSITION_BASIS_OFFSET)?,
        };
        require_nonzero(&[value.market_account, value.owner, value.basis_id])?;
        if value.claim_count == 0
            || bytes.len()
                != liability_basis_vector_width_v2(
                    LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
                    value.claim_count,
                )?
        {
            return Err(LiabilityBasisStateErrorV2::InvalidLength);
        }
        Ok(value)
    }

    /// Read one exact balance atom from the runtime tail.
    pub fn balance(self, bytes: &[u8], claim_index: u32) -> Result<u64> {
        if Self::decode(bytes)? != self {
            return Err(LiabilityBasisStateErrorV2::NonCanonical);
        }
        read_claim_v2(
            bytes,
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            self.claim_count,
            claim_index,
        )
    }
}

/// Return the exact header plus `u64[claim_count]` byte width.
pub fn liability_basis_vector_width_v2(header: usize, claim_count: u32) -> Result<usize> {
    if claim_count == 0 {
        return Err(LiabilityBasisStateErrorV2::InvalidClaimCount);
    }
    usize::try_from(claim_count)
        .ok()
        .and_then(|count| count.checked_mul(8))
        .and_then(|tail| header.checked_add(tail))
        .ok_or(LiabilityBasisStateErrorV2::InvalidClaimCount)
}

/// Encode one aggregate into an exact caller-owned slice, failure-atomically.
pub fn encode_liability_basis_market_into_v2(
    input: LiabilityBasisMarketInputV2,
    supplies: &[u64],
    output: &mut [u8],
) -> Result<()> {
    require_nonzero(&[
        input.logical_market,
        input.release_set,
        input.registry_program,
        input.product_instance_id,
        input.basis_id,
        input.realm_id,
        input.custody_context,
    ])?;
    let claim_count =
        u32::try_from(supplies.len()).map_err(|_| LiabilityBasisStateErrorV2::InvalidClaimCount)?;
    if output.len()
        != liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, claim_count)?
    {
        return Err(LiabilityBasisStateErrorV2::InvalidLength);
    }
    output.fill(0);
    put(output, 0, &LIABILITY_BASIS_MARKET_MAGIC_V2);
    put(output, 8, &LIABILITY_BASIS_STATE_VERSION_V2.to_le_bytes());
    put(
        output,
        MARKET_CLAIM_COUNT_OFFSET,
        &claim_count.to_le_bytes(),
    );
    put(
        output,
        MARKET_REVISION_OFFSET,
        &input.revision.to_le_bytes(),
    );
    for (offset, value) in [
        (MARKET_LOGICAL_ID_OFFSET, input.logical_market),
        (MARKET_RELEASE_SET_OFFSET, input.release_set),
        (MARKET_REGISTRY_OFFSET, input.registry_program),
        (MARKET_PRODUCT_OFFSET, input.product_instance_id),
        (MARKET_BASIS_OFFSET, input.basis_id),
        (MARKET_REALM_OFFSET, input.realm_id),
        (MARKET_CUSTODY_CONTEXT_OFFSET, input.custody_context),
    ] {
        put(output, offset, &value);
    }
    put(
        output,
        MARKET_GENERATION_OFFSET,
        &input.generation.to_le_bytes(),
    );
    write_claims(output, LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, supplies);
    LiabilityBasisMarketViewV2::decode(output).map(|_| ())
}

/// Encode one Position into an exact caller-owned slice, failure-atomically.
pub fn encode_liability_basis_position_into_v2(
    input: LiabilityBasisPositionInputV2,
    balances: &[u64],
    output: &mut [u8],
) -> Result<()> {
    require_nonzero(&[input.market_account, input.owner, input.basis_id])?;
    let claim_count =
        u32::try_from(balances.len()).map_err(|_| LiabilityBasisStateErrorV2::InvalidClaimCount)?;
    if output.len()
        != liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, claim_count)?
    {
        return Err(LiabilityBasisStateErrorV2::InvalidLength);
    }
    output.fill(0);
    put(output, 0, &LIABILITY_BASIS_POSITION_MAGIC_V2);
    put(output, 8, &LIABILITY_BASIS_STATE_VERSION_V2.to_le_bytes());
    put(
        output,
        POSITION_CLAIM_COUNT_OFFSET,
        &claim_count.to_le_bytes(),
    );
    put(
        output,
        POSITION_REVISION_OFFSET,
        &input.revision.to_le_bytes(),
    );
    put(output, POSITION_MARKET_OFFSET, &input.market_account);
    put(output, POSITION_OWNER_OFFSET, &input.owner);
    put(output, POSITION_BASIS_OFFSET, &input.basis_id);
    write_claims(output, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, balances);
    LiabilityBasisPositionViewV2::decode(output).map(|_| ())
}

/// Read one exact claim atom from an authenticated runtime-width vector.
pub fn read_claim_v2(
    bytes: &[u8],
    header: usize,
    claim_count: u32,
    claim_index: u32,
) -> Result<u64> {
    if claim_index >= claim_count
        || bytes.len() != liability_basis_vector_width_v2(header, claim_count)?
    {
        return Err(LiabilityBasisStateErrorV2::InvalidClaimIndex);
    }
    let offset = usize::try_from(claim_index)
        .ok()
        .and_then(|index| index.checked_mul(8))
        .and_then(|relative| header.checked_add(relative))
        .ok_or(LiabilityBasisStateErrorV2::InvalidClaimIndex)?;
    read_u64(bytes, offset)
}

fn require_prefix(bytes: &[u8], magic: [u8; 8], header: usize) -> Result<()> {
    if bytes.len() < header {
        return Err(LiabilityBasisStateErrorV2::InvalidLength);
    }
    if read_array::<8>(bytes, 0)? != magic {
        return Err(LiabilityBasisStateErrorV2::InvalidMagic);
    }
    if read_u16(bytes, 8)? != LIABILITY_BASIS_STATE_VERSION_V2 {
        return Err(LiabilityBasisStateErrorV2::UnsupportedVersion);
    }
    Ok(())
}

fn require_nonzero(values: &[[u8; 32]]) -> Result<()> {
    if values
        .iter()
        .any(|value| value.iter().all(|byte| *byte == 0))
    {
        Err(LiabilityBasisStateErrorV2::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn require_zero(bytes: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset
        .checked_add(width)
        .ok_or(LiabilityBasisStateErrorV2::InvalidLength)?;
    if bytes
        .get(offset..end)
        .ok_or(LiabilityBasisStateErrorV2::InvalidLength)?
        .iter()
        .all(|byte| *byte == 0)
    {
        Ok(())
    } else {
        Err(LiabilityBasisStateErrorV2::NonCanonical)
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(bytes, offset)?))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset
        .checked_add(N)
        .ok_or(LiabilityBasisStateErrorV2::InvalidLength)?;
    bytes
        .get(offset..end)
        .ok_or(LiabilityBasisStateErrorV2::InvalidLength)?
        .try_into()
        .map_err(|_| LiabilityBasisStateErrorV2::InvalidLength)
}

fn write_claims(output: &mut [u8], header: usize, values: &[u64]) {
    for (index, value) in values.iter().copied().enumerate() {
        if let Some(offset) = index
            .checked_mul(8)
            .and_then(|value| header.checked_add(value))
        {
            put(output, offset, &value.to_le_bytes());
        }
    }
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(end) = offset.checked_add(value.len())
        && let Some(destination) = output.get_mut(offset..end)
    {
        destination.copy_from_slice(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn market_input() -> LiabilityBasisMarketInputV2 {
        LiabilityBasisMarketInputV2 {
            revision: 7,
            logical_market: [1; 32],
            release_set: [2; 32],
            registry_program: [3; 32],
            product_instance_id: [4; 32],
            basis_id: [5; 32],
            realm_id: [6; 32],
            custody_context: [7; 32],
            generation: 8,
        }
    }

    #[test]
    fn exact_market_and_position_roundtrip() {
        let mut market = [0_u8; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 24];
        encode_liability_basis_market_into_v2(market_input(), &[11, 12, 13], &mut market)
            .expect("market");
        let market_view = LiabilityBasisMarketViewV2::decode(&market).expect("view");
        assert_eq!(market_view.claim_count, 3);
        assert_eq!(market_view.supply(&market, 2), Ok(13));

        let input = LiabilityBasisPositionInputV2 {
            revision: 9,
            market_account: [8; 32],
            owner: [9; 32],
            basis_id: [5; 32],
        };
        let mut position = [0_u8; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 24];
        encode_liability_basis_position_into_v2(input, &[4, 5, 6], &mut position)
            .expect("position");
        let position_view = LiabilityBasisPositionViewV2::decode(&position).expect("view");
        assert_eq!(position_view.balance(&position, 1), Ok(5));
    }

    #[test]
    fn hostile_identity_reserved_width_and_index_refuse() {
        let mut canonical = [0_u8; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 16];
        encode_liability_basis_market_into_v2(market_input(), &[1, 2], &mut canonical)
            .expect("market");
        let mut zero = canonical;
        zero.get_mut(MARKET_BASIS_OFFSET..MARKET_BASIS_OFFSET + 32)
            .expect("basis range")
            .fill(0);
        assert_eq!(
            LiabilityBasisMarketViewV2::decode(&zero),
            Err(LiabilityBasisStateErrorV2::ZeroIdentity)
        );
        let mut reserved = canonical;
        *reserved.get_mut(10).expect("reserved byte") = 1;
        assert_eq!(
            LiabilityBasisMarketViewV2::decode(&reserved),
            Err(LiabilityBasisStateErrorV2::NonCanonical)
        );
        assert_eq!(
            LiabilityBasisMarketViewV2::decode(
                canonical
                    .get(..canonical.len() - 1)
                    .expect("short hostile bytes"),
            ),
            Err(LiabilityBasisStateErrorV2::InvalidLength)
        );
        let view = LiabilityBasisMarketViewV2::decode(&canonical).expect("view");
        assert_eq!(
            view.supply(&canonical, 2),
            Err(LiabilityBasisStateErrorV2::InvalidClaimIndex)
        );
    }

    #[test]
    fn short_output_refuses_without_partial_write() {
        let mut output = [0xa5_u8; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 7];
        let before = output;
        assert_eq!(
            encode_liability_basis_market_into_v2(market_input(), &[1], &mut output),
            Err(LiabilityBasisStateErrorV2::InvalidLength)
        );
        assert_eq!(output, before);
    }

    #[test]
    fn public_layout_coordinates_track_encoders_and_round_trip() {
        let mut market = [0_u8; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 16];
        encode_liability_basis_market_into_v2(market_input(), &[11, 12], &mut market)
            .expect("market");
        assert_eq!(
            market.get(
                LiabilityBasisMarketLayoutV2::REVISION..LiabilityBasisMarketLayoutV2::REVISION + 8
            ),
            Some(7_u64.to_le_bytes().as_slice())
        );
        for (offset, expected) in [
            (
                LiabilityBasisMarketLayoutV2::LOGICAL_MARKET,
                market_input().logical_market,
            ),
            (
                LiabilityBasisMarketLayoutV2::RELEASE_SET,
                market_input().release_set,
            ),
            (
                LiabilityBasisMarketLayoutV2::REGISTRY_PROGRAM,
                market_input().registry_program,
            ),
            (
                LiabilityBasisMarketLayoutV2::PRODUCT_INSTANCE,
                market_input().product_instance_id,
            ),
            (LiabilityBasisMarketLayoutV2::BASIS, market_input().basis_id),
            (LiabilityBasisMarketLayoutV2::REALM, market_input().realm_id),
            (
                LiabilityBasisMarketLayoutV2::CUSTODY_CONTEXT,
                market_input().custody_context,
            ),
        ] {
            assert_eq!(market.get(offset..offset + 32), Some(expected.as_slice()));
        }
        assert_eq!(
            market.get(
                LiabilityBasisMarketLayoutV2::GENERATION
                    ..LiabilityBasisMarketLayoutV2::GENERATION + 8
            ),
            Some(market_input().generation.to_le_bytes().as_slice())
        );
        assert_eq!(
            market.get(
                LiabilityBasisMarketLayoutV2::SUPPLIES + LiabilityBasisMarketLayoutV2::SUPPLY_STRIDE
                    ..LiabilityBasisMarketLayoutV2::SUPPLIES
                        + 2 * LiabilityBasisMarketLayoutV2::SUPPLY_STRIDE
            ),
            Some(12_u64.to_le_bytes().as_slice())
        );
        let mut market_revision = u64::MAX;
        LiabilityBasisMarketLayoutV2::copy_revision_into(&market, &mut market_revision)
            .expect("market revision");
        assert_eq!(market_revision, 7);

        let input = LiabilityBasisPositionInputV2 {
            revision: 9,
            market_account: [8; 32],
            owner: [9; 32],
            basis_id: [5; 32],
        };
        let mut position = [0_u8; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 16];
        encode_liability_basis_position_into_v2(input, &[4, 5], &mut position).expect("position");
        assert_eq!(
            position.get(
                LiabilityBasisPositionLayoutV2::REVISION
                    ..LiabilityBasisPositionLayoutV2::REVISION + 8
            ),
            Some(9_u64.to_le_bytes().as_slice())
        );
        for (offset, expected) in [
            (LiabilityBasisPositionLayoutV2::MARKET, input.market_account),
            (LiabilityBasisPositionLayoutV2::OWNER, input.owner),
            (LiabilityBasisPositionLayoutV2::BASIS, input.basis_id),
        ] {
            assert_eq!(position.get(offset..offset + 32), Some(expected.as_slice()));
        }
        assert_eq!(
            position.get(
                LiabilityBasisPositionLayoutV2::BALANCES
                    + LiabilityBasisPositionLayoutV2::BALANCE_STRIDE
                    ..LiabilityBasisPositionLayoutV2::BALANCES
                        + 2 * LiabilityBasisPositionLayoutV2::BALANCE_STRIDE
            ),
            Some(5_u64.to_le_bytes().as_slice())
        );
        let mut position_revision = u64::MAX;
        LiabilityBasisPositionLayoutV2::copy_revision_into(&position, &mut position_revision)
            .expect("position revision");
        assert_eq!(position_revision, 9);
    }

    #[test]
    fn hostile_layout_projection_preserves_outputs() {
        let mut market = [0_u8; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 8];
        encode_liability_basis_market_into_v2(market_input(), &[11], &mut market).expect("market");
        market[10] = 1;
        let mut market_revision = 0xa5a5;
        let market_before = market_revision;
        assert!(
            LiabilityBasisMarketLayoutV2::copy_revision_into(&market, &mut market_revision)
                .is_err()
        );
        assert_eq!(market_revision, market_before);

        let input = LiabilityBasisPositionInputV2 {
            revision: 9,
            market_account: [8; 32],
            owner: [9; 32],
            basis_id: [5; 32],
        };
        let mut position = [0_u8; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 8];
        encode_liability_basis_position_into_v2(input, &[4], &mut position).expect("position");
        position[POSITION_RESERVED_OFFSET] = 1;
        let mut position_revision = 0x5a5a;
        let position_before = position_revision;
        assert!(
            LiabilityBasisPositionLayoutV2::copy_revision_into(&position, &mut position_revision)
                .is_err()
        );
        assert_eq!(position_revision, position_before);
    }
}
