//! Trading-owned outstanding Structured resource obligations.
//!
//! One obligation is created by each successful native receipt or coordinate
//! activation, and discharged only by its corresponding native retirement.
//! Coordinate retirement includes its shard, custody, Position and Admission.
//! The count spans every descriptor admitted by the immutable capability.

/// Canonical root-tail width, including its u64 obligation count.
pub const STRUCTURED_CAPABILITY_ROOT_BYTES_V2: usize = 24;
/// Domain preimage for the root-tail schema, including both obligation kinds.
pub const STRUCTURED_CAPABILITY_ROOT_SCHEMA_PREIMAGE_V2: &[u8] =
    b"dclutch/schema/structured-capability-root-v2;u64-outstanding-receipt-and-coordinate-groups";
/// Domain-separated root-tail schema.
pub const STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V2: [u8; 32] = [
    0x11, 0x97, 0xe0, 0xe7, 0xaa, 0x42, 0x26, 0x89, 0x7b, 0x99, 0x3d, 0x61, 0xac, 0x3d, 0x7b, 0x00,
    0x0f, 0x33, 0xd1, 0x66, 0xdb, 0x86, 0x6d, 0x81, 0x07, 0x4b, 0x7c, 0xd4, 0xc3, 0xe9, 0x1b, 0x59,
];
/// Count offset within the tail, excluding CapabilityRootHeaderV1.
pub const STRUCTURED_RESOURCE_GROUP_COUNT_OFFSET_V2: usize = 16;
/// Canonical newly activated root, with no outstanding resource groups.
pub const STRUCTURED_CAPABILITY_ROOT_TAIL_V2: [u8; STRUCTURED_CAPABILITY_ROOT_BYTES_V2] = [
    b'D', b'C', b'S', b'T', b'C', b'R', b'T', b'2', 2, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
/// Exact immutable magic word authenticated by lifecycle artifacts.
pub const STRUCTURED_CAPABILITY_ROOT_MAGIC_WORD_V2: u64 = u64::from_le_bytes(*b"DCSTCRT2");
/// Exact immutable version/active/reserved word authenticated by artifacts.
pub const STRUCTURED_CAPABILITY_ROOT_HEADER_WORD_V2: u64 = 0x0001_0002;

/// Canonical root-tail or checked obligation-count refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredRootErrorV2 {
    /// Wrong width or immutable header.
    Encoding,
    /// An activation would exceed the mathematical u64 representation.
    Overflow,
    /// No obligation exists to retire.
    Underflow,
    /// At least one activated resource group remains outstanding.
    OutstandingResources,
}

/// The sole persisted Structured resource-obligation count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StructuredCapabilityRootV2 {
    outstanding: u64,
}

impl StructuredCapabilityRootV2 {
    /// Decode exact canonical bytes, including all reserved header bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, StructuredRootErrorV2> {
        if bytes.len() != STRUCTURED_CAPABILITY_ROOT_BYTES_V2
            || bytes.get(..16) != Some(&STRUCTURED_CAPABILITY_ROOT_TAIL_V2[..16])
        {
            return Err(StructuredRootErrorV2::Encoding);
        }
        let count = bytes
            .get(16..24)
            .and_then(|v| v.try_into().ok())
            .ok_or(StructuredRootErrorV2::Encoding)?;
        Ok(Self {
            outstanding: u64::from_le_bytes(count),
        })
    }

    /// Encode the canonical immutable header and current obligation count.
    pub fn encode(self) -> [u8; STRUCTURED_CAPABILITY_ROOT_BYTES_V2] {
        let mut bytes = STRUCTURED_CAPABILITY_ROOT_TAIL_V2;
        bytes[16..24].copy_from_slice(&self.outstanding.to_le_bytes());
        bytes
    }

    /// Number of successfully activated, not yet retired resource groups.
    pub const fn outstanding(self) -> u64 {
        self.outstanding
    }

    /// Successor committed atomically with successful native activation.
    pub fn activate(self) -> Result<Self, StructuredRootErrorV2> {
        Ok(Self {
            outstanding: self
                .outstanding
                .checked_add(1)
                .ok_or(StructuredRootErrorV2::Overflow)?,
        })
    }

    /// Successor committed atomically with successful native retirement.
    pub fn retire(self) -> Result<Self, StructuredRootErrorV2> {
        Ok(Self {
            outstanding: self
                .outstanding
                .checked_sub(1)
                .ok_or(StructuredRootErrorV2::Underflow)?,
        })
    }

    /// Refuse physical root closure while any resource group remains.
    pub fn require_closeable(self) -> Result<(), StructuredRootErrorV2> {
        if self.outstanding != 0 {
            return Err(StructuredRootErrorV2::OutstandingResources);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_root_codec_rejects_every_header_substitution_and_integer_boundary() {
        assert_eq!(
            dclutch_sha256_adapter::digest(STRUCTURED_CAPABILITY_ROOT_SCHEMA_PREIMAGE_V2),
            STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V2
        );
        let initial = StructuredCapabilityRootV2::decode(&STRUCTURED_CAPABILITY_ROOT_TAIL_V2)
            .expect("initial");
        assert_eq!(initial.require_closeable(), Ok(()));
        assert_eq!(initial.retire(), Err(StructuredRootErrorV2::Underflow));
        let active = initial.activate().expect("activation");
        assert_eq!(
            StructuredCapabilityRootV2::decode(&active.encode()),
            Ok(active)
        );
        assert_eq!(
            active.require_closeable(),
            Err(StructuredRootErrorV2::OutstandingResources)
        );
        for offset in 0..16 {
            let mut hostile = active.encode();
            hostile[offset] ^= 1;
            assert_eq!(
                StructuredCapabilityRootV2::decode(&hostile),
                Err(StructuredRootErrorV2::Encoding)
            );
        }
        assert_eq!(
            StructuredCapabilityRootV2::decode(&active.encode()[..23]),
            Err(StructuredRootErrorV2::Encoding)
        );
        let mut maximum = initial.encode();
        maximum[16..24].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            StructuredCapabilityRootV2::decode(&maximum)
                .expect("representable maximum")
                .activate(),
            Err(StructuredRootErrorV2::Overflow)
        );
    }
}
