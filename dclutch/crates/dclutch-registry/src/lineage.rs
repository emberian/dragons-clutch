//! Release-set lineage: the one record that names a set's successor.
//!
//! A release set is content-addressed and immutable, so it cannot grow a
//! pointer to a successor that did not exist when it was hashed, and a
//! successor cannot be trusted to name its own predecessor because the
//! authority that must consent is the *predecessor's*. The link therefore has
//! no home inside either endpoint and lives in its own account, keyed by the
//! PREDECESSOR.
//!
//! Keying by the predecessor buys the no-fork guarantee structurally: a second
//! declaration for the same predecessor finds the account already created and
//! refuses before this codec is ever called. No code implements that rule.
//!
//! The record is also the whole story for a reader who holds nothing else.
//! Cache closure (Q8c) decodes a lineage record without either endpoint's cache
//! in its frame, so `decode` refuses every internally incoherent record here
//! rather than leaving that reader to re-derive the coherence it cannot see.

use crate::release_set::{EXECUTION_ROLE_COUNT_V1, EXECUTION_ROLE_ORDER_V1, ExecutionRoleV1};
use dclutch_core_contract::ContentId;

use crate::{
    Error, IDENTITY_BYTES, Result, copy_infallible, put_u16, read_array, read_byte, read_u16,
    require_zero,
};

/// First PDA seed for a release-set lineage record, keyed by PREDECESSOR.
///
/// The adapter must derive the record under the Registry program with exactly
/// `[RELEASE_LINEAGE_PDA_DOMAIN_V1, predecessor_release_set_id]`, in that
/// order, and no caller-selected seed. This is the two-seed shape of the
/// activation cache, so a reader who knows one derivation knows both.
pub const RELEASE_LINEAGE_PDA_DOMAIN_V1: &[u8; 26] = b"dclutch:release-lineage:v1";

/// Seeds in the lineage PDA projection, excluding the bump.
pub const RELEASE_LINEAGE_PDA_SEED_COUNT_V1: usize = 2;

// The domain's width is part of the address, so it is asserted rather than
// trusted to the literal above staying the length its type claims.
const _: () = assert!(
    RELEASE_LINEAGE_PDA_DOMAIN_V1.len() == 26,
    "release-lineage PDA domain must keep its exact seed width"
);
// Both Registry domains take the same second seed, so an equal domain would
// derive one address for two different records. Unequal widths is a proof of
// unequal bytes, and it is the one a const can carry.
const _: () = assert!(
    RELEASE_LINEAGE_PDA_DOMAIN_V1.len() != crate::ACTIVATION_PDA_DOMAIN_V1.len(),
    "release-lineage and activation PDA domains must stay distinguishable"
);

/// Bytes in one complete release-set lineage record.
pub const RELEASE_LINEAGE_BYTES_V1: usize = 248;
/// Canonical release-set lineage magic.
pub const RELEASE_LINEAGE_MAGIC_V1: [u8; 8] = *b"DCLTRLN1";
/// Implemented release-set lineage schema.
pub const RELEASE_LINEAGE_SCHEMA_VERSION_V1: u16 = 1;
/// Implemented release-set lineage fixed-layout profile.
pub const RELEASE_LINEAGE_PROFILE_V1: u16 = 1;

const SCHEMA_OFFSET: usize = 8;
const PROFILE_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 12;
const HEADER_RESERVED_BYTES: usize = 4;
const PREDECESSOR_OFFSET: usize = 16;
const SUCCESSOR_OFFSET: usize = 48;
const MOVED_ROLES_OFFSET: usize = 80;
const MOVED_RESERVED_OFFSET: usize = 85;
const MOVED_RESERVED_BYTES: usize = 3;
const AUTHORITIES_OFFSET: usize = 88;

const MOVED_BYTE: u8 = 1;
const UNMOVED_BYTE: u8 = 0;

// The declared width is the sum of the fields, not a number someone typed.
const _: () = assert!(
    RELEASE_LINEAGE_BYTES_V1 == AUTHORITIES_OFFSET + EXECUTION_ROLE_COUNT_V1 * IDENTITY_BYTES,
    "release-lineage width must equal its own layout"
);
const _: () = assert!(
    MOVED_ROLES_OFFSET + EXECUTION_ROLE_COUNT_V1 == MOVED_RESERVED_OFFSET,
    "the moved-role mask must hold exactly one byte per canonical role"
);
const _: () = assert!(
    MOVED_RESERVED_OFFSET + MOVED_RESERVED_BYTES == AUTHORITIES_OFFSET,
    "the moved-role reserved run must close exactly at the authority table"
);

/// One release set's declared successor, and who consented to the hop.
///
/// Consent is stored per role as one `Option`: `Some(authority)` is "this
/// role's artifact moved, and that key signed for it", `None` is "this role's
/// binding is byte-identical on both sides, so it makes no new claim and no
/// consent was asked for". Holding the mask and the key as one field is what
/// makes a record claiming an unconsented move unrepresentable rather than
/// merely refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseLineageV1 {
    predecessor: ContentId,
    successor: ContentId,
    consent: [Option<[u8; IDENTITY_BYTES]>; EXECUTION_ROLE_COUNT_V1],
}

impl ReleaseLineageV1 {
    /// Construct and validate one release-set lineage record.
    ///
    /// `consent` is indexed by [`ExecutionRoleV1::role_index`], never by a
    /// restated order.
    pub fn new(
        predecessor: ContentId,
        successor: ContentId,
        consent: [Option<[u8; IDENTITY_BYTES]>; EXECUTION_ROLE_COUNT_V1],
    ) -> Result<Self> {
        if predecessor == successor {
            return Err(Error::LineageSelfSuccession);
        }
        let mut moved_any = false;
        for authority in consent.iter().flatten() {
            // A zero key is the record's own "no consent" value, so a `Some`
            // holding one would encode as an unmoved role and read back as a
            // different record than the one that was constructed.
            if authority.iter().all(|byte| *byte == 0) {
                return Err(Error::NonCanonicalLineageConsent);
            }
            moved_any = true;
        }
        if !moved_any {
            return Err(Error::LineageWithoutMovedRole);
        }
        Ok(Self {
            predecessor,
            successor,
            consent,
        })
    }

    /// Hostile-decode one exact release-set lineage record.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        validate_lineage_header(bytes)?;
        let predecessor = ContentId::new(read_array(bytes, PREDECESSOR_OFFSET)?)
            .map_err(|_| Error::ZeroIdentity)?;
        let successor = ContentId::new(read_array(bytes, SUCCESSOR_OFFSET)?)
            .map_err(|_| Error::ZeroIdentity)?;
        let mut consent = [None; EXECUTION_ROLE_COUNT_V1];
        for role in EXECUTION_ROLE_ORDER_V1 {
            let index = role.role_index();
            let moved = match read_byte(bytes, MOVED_ROLES_OFFSET + index)? {
                UNMOVED_BYTE => false,
                MOVED_BYTE => true,
                _ => return Err(Error::NonCanonicalLineageConsent),
            };
            let authority: [u8; IDENTITY_BYTES] =
                read_array(bytes, AUTHORITIES_OFFSET + index * IDENTITY_BYTES)?;
            let present = authority.iter().any(|byte| *byte != 0);
            // The mask and the key are one fact. A record that says a role
            // moved but records nobody's consent, or records a key for a role
            // that did not move, is not a record this type can hold.
            if moved != present {
                return Err(Error::NonCanonicalLineageConsent);
            }
            if let Some(slot) = consent.get_mut(index) {
                *slot = moved.then_some(authority);
            }
        }
        Self::new(predecessor, successor, consent)
    }

    /// Encode the one canonical release-set lineage preimage.
    pub fn to_bytes(self) -> [u8; RELEASE_LINEAGE_BYTES_V1] {
        let mut output = [0; RELEASE_LINEAGE_BYTES_V1];
        copy_infallible(&mut output, 0, &RELEASE_LINEAGE_MAGIC_V1);
        put_u16(
            &mut output,
            SCHEMA_OFFSET,
            RELEASE_LINEAGE_SCHEMA_VERSION_V1,
        );
        put_u16(&mut output, PROFILE_OFFSET, RELEASE_LINEAGE_PROFILE_V1);
        copy_infallible(&mut output, PREDECESSOR_OFFSET, self.predecessor.as_bytes());
        copy_infallible(&mut output, SUCCESSOR_OFFSET, self.successor.as_bytes());
        for role in EXECUTION_ROLE_ORDER_V1 {
            let index = role.role_index();
            let Some(Some(authority)) = self.consent.get(index) else {
                continue;
            };
            if let Some(mask) = output.get_mut(MOVED_ROLES_OFFSET + index) {
                *mask = MOVED_BYTE;
            }
            copy_infallible(
                &mut output,
                AUTHORITIES_OFFSET + index * IDENTITY_BYTES,
                authority,
            );
        }
        output
    }

    /// Return the release set this record is keyed by.
    pub const fn predecessor(self) -> ContentId {
        self.predecessor
    }

    /// Return the release set a market on the predecessor migrates to.
    pub const fn successor(self) -> ContentId {
        self.successor
    }

    /// Return the key that consented for this role, if its artifact moved.
    pub fn consenting_authority(self, role: ExecutionRoleV1) -> Option<[u8; IDENTITY_BYTES]> {
        self.consent.get(role.role_index()).copied().flatten()
    }

    /// Whether this role's artifact release changed across the hop.
    pub fn moved(self, role: ExecutionRoleV1) -> bool {
        self.consenting_authority(role).is_some()
    }
}

fn validate_lineage_header(bytes: &[u8]) -> Result<()> {
    if bytes.len() != RELEASE_LINEAGE_BYTES_V1 {
        return Err(Error::InvalidLength);
    }
    if bytes.get(..RELEASE_LINEAGE_MAGIC_V1.len()) != Some(RELEASE_LINEAGE_MAGIC_V1.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    if read_u16(bytes, SCHEMA_OFFSET)? != RELEASE_LINEAGE_SCHEMA_VERSION_V1 {
        return Err(Error::UnsupportedSchema);
    }
    if read_u16(bytes, PROFILE_OFFSET)? != RELEASE_LINEAGE_PROFILE_V1 {
        return Err(Error::UnsupportedArtifactProfile);
    }
    require_zero(bytes, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
    require_zero(bytes, MOVED_RESERVED_OFFSET, MOVED_RESERVED_BYTES)
}
