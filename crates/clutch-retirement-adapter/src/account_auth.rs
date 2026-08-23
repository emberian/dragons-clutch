// SPDX-License-Identifier: AGPL-3.0-or-later

use clutch_retirement::{
    Identity32V1, RetirementErrorV1, DIRECT_RESERVATION_ACCOUNT_VERSION_V6,
    DIRECT_RESERVATION_V6_BYTES, EPOCH_ACCOUNT_TAG, EPOCH_ACCOUNT_VERSION_V5, EPOCH_V5_BYTES,
    GENERAL_EPOCH_TOMBSTONE_TAG, GENERAL_EPOCH_TOMBSTONE_V1_BYTES,
    GENERAL_EPOCH_TOMBSTONE_VERSION_V1, MARKET_ACCOUNT_TAG, MARKET_ACCOUNT_VERSION_V2,
    MARKET_V2_BYTES, POSITION_ACCOUNT_TAG, POSITION_ACCOUNT_VERSION_V2, POSITION_TOMBSTONE_TAG,
    POSITION_TOMBSTONE_V1_BYTES, POSITION_TOMBSTONE_VERSION_V1, POSITION_V2_BYTES,
    RESERVATION_ACCOUNT_TAG, RESERVATION_ACCOUNT_VERSION_V5, RESERVATION_V5_BYTES,
};

use crate::RetirementAdapterErrorV1;

const POSITION_STORED_BUMP_OFFSET: usize = 218;
const MARKET_STORED_BUMP_OFFSET: usize = 132;
const EPOCH_STORED_BUMP_OFFSET: usize = 327;
const RESERVATION_STORED_BUMP_OFFSET: usize = 312;
const POSITION_TOMBSTONE_STORED_BUMP_OFFSET: usize = 75;
const EPOCH_TOMBSTONE_STORED_BUMP_OFFSET: usize = 83;

/// Runtime facts read from one Solana account before any state mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountViewV1<'a> {
    /// Actual account address.
    pub address: Identity32V1,
    /// Actual runtime owner program.
    pub owner: Identity32V1,
    /// Entire current account data slice.
    pub data: &'a [u8],
    /// Whether the transaction declared the account writable.
    pub is_writable: bool,
}

/// Canonical PDA output already derived from exact seeds by the Solana adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPdaV1 {
    address: Identity32V1,
    bump: u8,
}

impl CanonicalPdaV1 {
    /// Construct only after the adapter derives the PDA from the instruction's
    /// exact seed schema and the authenticated program id.
    pub const fn after_derivation(address: Identity32V1, bump: u8) -> Self {
        Self { address, bump }
    }

    /// Derived address.
    pub const fn address(self) -> Identity32V1 {
        self.address
    }

    /// Derived canonical bump.
    pub const fn bump(self) -> u8 {
        self.bump
    }
}

/// A runtime view that passed address, owner, mutability, exact-header,
/// exact-length, and stored-bump checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedAccountV1<'a> {
    view: AccountViewV1<'a>,
    canonical_bump: u8,
}

impl<'a> AuthenticatedAccountV1<'a> {
    /// Exact authenticated data passed to a semantic codec next.
    pub const fn data(self) -> &'a [u8] {
        self.view.data
    }

    /// Authenticated canonical address.
    pub const fn address(self) -> Identity32V1 {
        self.view.address
    }

    /// Canonical derived and stored bump.
    pub const fn bump(self) -> u8 {
        self.canonical_bump
    }
}

/// Registry-supplied shape for a version-bumped child with an eight-byte
/// parent-generation tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CountedChildSchemaV1 {
    tag: u8,
    legacy_version: u8,
    counted_version: u8,
    legacy_len: usize,
    counted_len: usize,
    stored_bump_offset: usize,
}

impl CountedChildSchemaV1 {
    /// Construct only from a globally allocated tag/version pair and the
    /// authoritative legacy codec's exact length and bump offset.
    pub const fn after_registry_allocation(
        tag: u8,
        legacy_version: u8,
        counted_version: u8,
        legacy_len: usize,
        stored_bump_offset: usize,
    ) -> Result<Self, RetirementAdapterErrorV1> {
        if legacy_len < 2 || stored_bump_offset >= legacy_len || legacy_version == counted_version {
            return Err(RetirementAdapterErrorV1::InvalidSchema);
        }
        let counted_len = match legacy_len.checked_add(8) {
            Some(value) => value,
            None => return Err(RetirementAdapterErrorV1::InvalidSchema),
        };
        Ok(Self {
            tag,
            legacy_version,
            counted_version,
            legacy_len,
            counted_len,
            stored_bump_offset,
        })
    }

    pub(crate) const fn tag(self) -> u8 {
        self.tag
    }

    pub(crate) const fn legacy_version(self) -> u8 {
        self.legacy_version
    }

    pub(crate) const fn counted_version(self) -> u8 {
        self.counted_version
    }

    /// Exact legacy body width before appending the generation.
    pub const fn legacy_len(self) -> usize {
        self.legacy_len
    }

    /// Exact promoted width including the generation.
    pub const fn counted_len(self) -> usize {
        self.counted_len
    }
}

#[derive(Clone, Copy)]
struct ExpectedAccountV1 {
    tag: u8,
    version: u8,
    len: usize,
    bump_offset: usize,
}

fn authenticate<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    expected: ExpectedAccountV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    if view.address != canonical_pda.address {
        return Err(RetirementAdapterErrorV1::WrongPda);
    }
    if view.owner != program_id {
        return Err(RetirementAdapterErrorV1::WrongOwner);
    }
    if !view.is_writable {
        return Err(RetirementAdapterErrorV1::NotWritable);
    }
    if view.data.len() < expected.len {
        return Err(RetirementErrorV1::Truncated.into());
    }
    if view.data.len() > expected.len {
        return Err(RetirementErrorV1::TrailingBytes.into());
    }
    if expected.len < 2 || expected.bump_offset >= expected.len {
        return Err(RetirementAdapterErrorV1::InvalidSchema);
    }
    if view.data[0] != expected.tag {
        return Err(RetirementErrorV1::WrongTag.into());
    }
    if view.data[1] != expected.version {
        return Err(RetirementErrorV1::WrongVersion.into());
    }
    if view.data[expected.bump_offset] != canonical_pda.bump {
        return Err(RetirementAdapterErrorV1::WrongBump);
    }
    Ok(AuthenticatedAccountV1 {
        view,
        canonical_bump: canonical_pda.bump,
    })
}

/// Authenticate one writable counted Position V2 before decoding it.
pub fn authenticate_position_v2<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: POSITION_ACCOUNT_TAG,
            version: POSITION_ACCOUNT_VERSION_V2,
            len: POSITION_V2_BYTES,
            bump_offset: POSITION_STORED_BUMP_OFFSET,
        },
    )
}

/// Authenticate one writable monotone-cursor Market V2 before decoding it.
pub fn authenticate_market_v2<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: MARKET_ACCOUNT_TAG,
            version: MARKET_ACCOUNT_VERSION_V2,
            len: MARKET_V2_BYTES,
            bump_offset: MARKET_STORED_BUMP_OFFSET,
        },
    )
}

/// Authenticate one writable counted general Epoch V5 before decoding it.
pub fn authenticate_general_epoch_v5<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: EPOCH_ACCOUNT_TAG,
            version: EPOCH_ACCOUNT_VERSION_V5,
            len: EPOCH_V5_BYTES,
            bump_offset: EPOCH_STORED_BUMP_OFFSET,
        },
    )
}

/// Authenticate one writable counted general Reservation V5.
pub fn authenticate_general_reservation_v5<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: RESERVATION_ACCOUNT_TAG,
            version: RESERVATION_ACCOUNT_VERSION_V5,
            len: RESERVATION_V5_BYTES,
            bump_offset: RESERVATION_STORED_BUMP_OFFSET,
        },
    )
}

/// Authenticate one writable counted direct Reservation V6.
pub fn authenticate_direct_reservation_v6<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: RESERVATION_ACCOUNT_TAG,
            version: DIRECT_RESERVATION_ACCOUNT_VERSION_V6,
            len: DIRECT_RESERVATION_V6_BYTES,
            bump_offset: RESERVATION_STORED_BUMP_OFFSET,
        },
    )
}

/// Authenticate one writable permanent Position tombstone.
pub fn authenticate_position_tombstone_v1<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: POSITION_TOMBSTONE_TAG,
            version: POSITION_TOMBSTONE_VERSION_V1,
            len: POSITION_TOMBSTONE_V1_BYTES,
            bump_offset: POSITION_TOMBSTONE_STORED_BUMP_OFFSET,
        },
    )
}

/// Authenticate one writable permanent general Epoch tombstone.
pub fn authenticate_general_epoch_tombstone_v1<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: GENERAL_EPOCH_TOMBSTONE_TAG,
            version: GENERAL_EPOCH_TOMBSTONE_VERSION_V1,
            len: GENERAL_EPOCH_TOMBSTONE_V1_BYTES,
            bump_offset: EPOCH_TOMBSTONE_STORED_BUMP_OFFSET,
        },
    )
}

/// Authenticate one writable counted child using its globally allocated
/// schema. Its semantic owner must decode the downgraded base next.
pub fn authenticate_counted_child<'a>(
    view: AccountViewV1<'a>,
    program_id: Identity32V1,
    canonical_pda: CanonicalPdaV1,
    schema: CountedChildSchemaV1,
) -> Result<AuthenticatedAccountV1<'a>, RetirementAdapterErrorV1> {
    authenticate(
        view,
        program_id,
        canonical_pda,
        ExpectedAccountV1 {
            tag: schema.tag,
            version: schema.counted_version,
            len: schema.counted_len,
            bump_offset: schema.stored_bump_offset,
        },
    )
}

const _: () = assert!(POSITION_STORED_BUMP_OFFSET < POSITION_V2_BYTES);
const _: () = assert!(MARKET_STORED_BUMP_OFFSET < MARKET_V2_BYTES);
const _: () = assert!(EPOCH_STORED_BUMP_OFFSET < EPOCH_V5_BYTES);
const _: () = assert!(RESERVATION_STORED_BUMP_OFFSET < RESERVATION_V5_BYTES);
