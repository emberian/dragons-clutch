//! Family-neutral, read-only projection of one fully authenticated Found request.

use crate::{Action, Error as CoreError, Identity, REQUEST_BYTES, Request};

/// Exact ProjectFound instruction prefix.
pub const PROJECT_FOUND_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTPFQ1";
/// Exact ProjectFound instruction width.
pub const PROJECT_FOUND_REQUEST_BYTES_V1: usize = 16 + REQUEST_BYTES;
/// Exact ProjectFound return receipt magic.
pub const PROJECT_FOUND_RECEIPT_MAGIC_V1: [u8; 8] = *b"DCLTPFR1";
/// Exact ProjectFound return receipt width.
pub const PROJECT_FOUND_RECEIPT_BYTES_V1: usize = 404;

const VERSION_V1: u16 = 1;

/// Stable refusal from the ProjectFound physical ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectFoundError {
    /// The byte slice had the wrong exact width.
    InvalidLength,
    /// The byte slice selected another wire family.
    InvalidMagic,
    /// The byte slice selected another ABI version.
    InvalidVersion,
    /// Reserved or inactive bytes were nonzero.
    NonCanonical,
    /// A required identity or digest was zero.
    ZeroIdentity,
    /// The embedded Core request was not Found.
    NotFound,
    /// The embedded Core request was not canonical.
    CoreRequest,
    /// The receipt did not acknowledge the exact Found request.
    RequestMismatch,
}

impl From<CoreError> for ProjectFoundError {
    fn from(_: CoreError) -> Self {
        Self::CoreRequest
    }
}

/// Read-only request to authenticate and project one future Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectFoundRequestV1 {
    /// The exact ordinary Core Found request being projected.
    pub found: Request,
}

impl ProjectFoundRequestV1 {
    /// Construct a request only from a canonical Found action.
    pub fn new(found: Request) -> Result<Self, ProjectFoundError> {
        if found.action != Action::Found {
            return Err(ProjectFoundError::NotFound);
        }
        found.encode()?;
        Ok(Self { found })
    }

    /// Hostile-decode one exact request.
    pub fn decode(input: &[u8]) -> Result<Self, ProjectFoundError> {
        exact_len(input, PROJECT_FOUND_REQUEST_BYTES_V1)?;
        exact_magic(input, &PROJECT_FOUND_REQUEST_MAGIC_V1)?;
        if read_u16(input, 8)? != VERSION_V1 || any_nonzero(input, 10, 6)? {
            return Err(ProjectFoundError::NonCanonical);
        }
        let found = Request::decode(slice(input, 16, REQUEST_BYTES)?)?;
        Self::new(found)
    }

    /// Encode the sole canonical request bytes.
    pub fn encode(self) -> Result<[u8; PROJECT_FOUND_REQUEST_BYTES_V1], ProjectFoundError> {
        let value = Self::new(self.found)?;
        let mut output = [0; PROJECT_FOUND_REQUEST_BYTES_V1];
        write(&mut output, 0, &PROJECT_FOUND_REQUEST_MAGIC_V1)?;
        write(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        write(&mut output, 16, &value.found.encode()?)?;
        Ok(output)
    }
}

/// Immediate Core-produced projection of one future Market identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectFoundReceiptV1 {
    /// Future Core Market PDA.
    pub market: Identity,
    /// Market generation.
    pub generation: u64,
    /// Immutable Realm content identity.
    pub realm: Identity,
    /// Realm-selected collateral Mint.
    pub collateral_mint: Identity,
    /// Realm-selected Token or Token-2022 program.
    pub token_program: Identity,
    /// Realm-selected immutable collateral-adapter release.
    pub collateral_release: Identity,
    /// Exact Product finalized-record content identity.
    pub product_record: Identity,
    /// Exact semantic Product identity.
    pub product: Identity,
    /// Exact Source resolution-policy content identity.
    pub source: Identity,
    /// Exact selected execution-release-set content identity.
    pub release_set: Identity,
    /// Immutable infrastructure-selected Rent program.
    pub rent_program: Identity,
    /// SHA-256 of the exact embedded canonical Core Found request.
    pub found_request_digest: [u8; 32],
}

impl ProjectFoundReceiptV1 {
    /// Construct one checked receipt.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        market: Identity,
        generation: u64,
        realm: Identity,
        collateral_mint: Identity,
        token_program: Identity,
        collateral_release: Identity,
        product_record: Identity,
        product: Identity,
        source: Identity,
        release_set: Identity,
        rent_program: Identity,
        found_request_digest: [u8; 32],
    ) -> Result<Self, ProjectFoundError> {
        if found_request_digest.iter().all(|byte| *byte == 0) {
            return Err(ProjectFoundError::ZeroIdentity);
        }
        Ok(Self {
            market,
            generation,
            realm,
            collateral_mint,
            token_program,
            collateral_release,
            product_record,
            product,
            source,
            release_set,
            rent_program,
            found_request_digest,
        })
    }

    /// Encode the sole canonical receipt.
    pub fn encode(self) -> Result<[u8; PROJECT_FOUND_RECEIPT_BYTES_V1], ProjectFoundError> {
        let value = Self::new(
            self.market,
            self.generation,
            self.realm,
            self.collateral_mint,
            self.token_program,
            self.collateral_release,
            self.product_record,
            self.product,
            self.source,
            self.release_set,
            self.rent_program,
            self.found_request_digest,
        )?;
        let mut output = [0; PROJECT_FOUND_RECEIPT_BYTES_V1];
        write(&mut output, 0, &PROJECT_FOUND_RECEIPT_MAGIC_V1)?;
        write(&mut output, 8, &VERSION_V1.to_le_bytes())?;
        write(&mut output, 16, &value.market.to_bytes())?;
        write(&mut output, 48, &value.generation.to_le_bytes())?;
        for (offset, identity) in [
            (56, value.realm),
            (88, value.collateral_mint),
            (120, value.token_program),
            (152, value.collateral_release),
            (184, value.product_record),
            (216, value.product),
            (248, value.source),
            (280, value.release_set),
            (312, value.rent_program),
        ] {
            write(&mut output, offset, &identity.to_bytes())?;
        }
        write(&mut output, 344, &value.found_request_digest)?;
        Ok(output)
    }

    /// Hostile-decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self, ProjectFoundError> {
        exact_len(input, PROJECT_FOUND_RECEIPT_BYTES_V1)?;
        exact_magic(input, &PROJECT_FOUND_RECEIPT_MAGIC_V1)?;
        if read_u16(input, 8)? != VERSION_V1
            || any_nonzero(input, 10, 6)?
            || any_nonzero(input, 376, 28)?
        {
            return Err(ProjectFoundError::NonCanonical);
        }
        Self::new(
            Identity::new(read_array(input, 16)?)?,
            read_u64(input, 48)?,
            Identity::new(read_array(input, 56)?)?,
            Identity::new(read_array(input, 88)?)?,
            Identity::new(read_array(input, 120)?)?,
            Identity::new(read_array(input, 152)?)?,
            Identity::new(read_array(input, 184)?)?,
            Identity::new(read_array(input, 216)?)?,
            Identity::new(read_array(input, 248)?)?,
            Identity::new(read_array(input, 280)?)?,
            Identity::new(read_array(input, 312)?)?,
            read_array(input, 344)?,
        )
    }

    /// Require acknowledgement of the exact canonical Found request digest.
    pub fn verify_found_request(self, digest: [u8; 32]) -> Result<(), ProjectFoundError> {
        if self.found_request_digest != digest {
            return Err(ProjectFoundError::RequestMismatch);
        }
        Ok(())
    }
}

fn exact_len(input: &[u8], expected: usize) -> Result<(), ProjectFoundError> {
    if input.len() == expected {
        Ok(())
    } else {
        Err(ProjectFoundError::InvalidLength)
    }
}

fn exact_magic(input: &[u8], magic: &[u8; 8]) -> Result<(), ProjectFoundError> {
    if input.get(..8) == Some(magic.as_slice()) {
        Ok(())
    } else {
        Err(ProjectFoundError::InvalidMagic)
    }
}

fn slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8], ProjectFoundError> {
    input
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(ProjectFoundError::InvalidLength)?,
        )
        .ok_or(ProjectFoundError::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), ProjectFoundError> {
    let end = offset
        .checked_add(value.len())
        .ok_or(ProjectFoundError::InvalidLength)?;
    let destination = output
        .get_mut(offset..end)
        .ok_or(ProjectFoundError::InvalidLength)?;
    destination.copy_from_slice(value);
    Ok(())
}

fn any_nonzero(input: &[u8], offset: usize, width: usize) -> Result<bool, ProjectFoundError> {
    Ok(slice(input, offset, width)?.iter().any(|byte| *byte != 0))
}

fn read_array(input: &[u8], offset: usize) -> Result<[u8; 32], ProjectFoundError> {
    slice(input, offset, 32)?
        .try_into()
        .map_err(|_| ProjectFoundError::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, ProjectFoundError> {
    Ok(u16::from_le_bytes(
        slice(input, offset, 2)?
            .try_into()
            .map_err(|_| ProjectFoundError::InvalidLength)?,
    ))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, ProjectFoundError> {
    Ok(u64::from_le_bytes(
        slice(input, offset, 8)?
            .try_into()
            .map_err(|_| ProjectFoundError::InvalidLength)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(seed: u8) -> Identity {
        Identity::new([seed; 32]).expect("nonzero")
    }

    #[test]
    fn project_request_and_receipt_are_canonical() {
        let found = Request::administrative(Action::Found, 9, id(1));
        let request = ProjectFoundRequestV1::new(found).expect("Found");
        let bytes = request.encode().expect("request");
        assert_eq!(ProjectFoundRequestV1::decode(&bytes), Ok(request));

        let receipt = ProjectFoundReceiptV1::new(
            id(1),
            9,
            id(2),
            id(3),
            id(4),
            id(5),
            id(6),
            id(7),
            id(8),
            id(9),
            id(10),
            [11; 32],
        )
        .expect("receipt");
        let bytes = receipt.encode().expect("receipt bytes");
        assert_eq!(ProjectFoundReceiptV1::decode(&bytes), Ok(receipt));
        assert_eq!(receipt.verify_found_request([11; 32]), Ok(()));
    }

    #[test]
    fn hostile_bytes_and_non_found_requests_refuse() {
        let open = Request::administrative(Action::OpenMarket, 9, id(1));
        assert_eq!(
            ProjectFoundRequestV1::new(open),
            Err(ProjectFoundError::NotFound)
        );
        let mut bytes =
            ProjectFoundRequestV1::new(Request::administrative(Action::Found, 9, id(1)))
                .expect("Found")
                .encode()
                .expect("bytes");
        bytes[10] = 1;
        assert_eq!(
            ProjectFoundRequestV1::decode(&bytes),
            Err(ProjectFoundError::NonCanonical)
        );
    }
}
