// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    CountedDealerChildV1, DealerChildKindV1, DeletableRentOwnerV1, Error, FixedCodec, Id, Result,
    DELETABLE_RENT_OWNER_BYTES, LP_ENTRIES_PER_PAGE, LP_PAGE_CONTENT_DOMAIN_V1, MAX_ATOMS,
    MAX_LP_PAGES, NO_NEXT_LP_PAGE,
};

/// Local semantic-body magic; this is not a global account discriminator.
pub const LP_PAGE_MAGIC_V1: [u8; 8] = *b"DCLPPGV1";
/// Exact local semantic-body version.
pub const LP_PAGE_VERSION_V1: u16 = 1;
/// Exact bytes in one fixed LP entry.
pub const LP_ENTRY_BYTES_V1: usize = 64;
/// Exact bytes in one canonical `LpPageV1` body.
pub const LP_PAGE_BYTES_V1: usize = HEADER_BYTES
    + (2 * 32)
    + 8
    + 8
    + 4
    + 8
    + (LP_ENTRIES_PER_PAGE * LP_ENTRY_BYTES_V1)
    + DELETABLE_RENT_OWNER_BYTES;

/// One canonical LP share, queue, and terminal-claim entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LpEntryV1 {
    /// Immutable LP owner; zero only for inactive fixed-width padding.
    pub owner: Id,
    /// Exact capital-unit shares owned by this identity.
    pub shares: u64,
    /// Irrevocably queued shares requesting unwind-only mode.
    pub queued_shares: u64,
    /// Exact terminal cash claim assigned by the canonical allocation.
    pub terminal_claim_atoms: u64,
    /// Whether the terminal claim, including a possible zero claim, was delivered.
    pub claimed: bool,
}

impl LpEntryV1 {
    /// Canonical inactive fixed-width entry.
    pub const EMPTY: Self = Self {
        owner: Id::ZERO,
        shares: 0,
        queued_shares: 0,
        terminal_claim_atoms: 0,
        claimed: false,
    };

    fn validate_live(&self, terminal_allocated: bool) -> Result<()> {
        self.owner.validate_live()?;
        if self.shares == 0
            || self.shares > MAX_ATOMS
            || self.queued_shares > self.shares
            || self.terminal_claim_atoms > 4 * MAX_ATOMS
            || (!terminal_allocated && (self.terminal_claim_atoms != 0 || self.claimed))
        {
            return Err(Error::InvalidLpPage);
        }
        Ok(())
    }

    fn encode_body(&self, writer: &mut Writer<'_>) {
        writer.id(self.owner);
        writer.u64(self.shares);
        writer.u64(self.queued_shares);
        writer.u64(self.terminal_claim_atoms);
        writer.bool(self.claimed);
        writer.reserved(7);
    }

    fn decode_body(reader: &mut Reader<'_>) -> Result<Self> {
        let value = Self {
            owner: reader.id(),
            shares: reader.u64(),
            queued_shares: reader.u64(),
            terminal_claim_atoms: reader.u64(),
            claimed: reader.bool()?,
        };
        reader.reserved(7)?;
        Ok(value)
    }
}

/// One page in the strictly owner-sorted LP ownership set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LpPageV1 {
    /// Canonical `DealerPolicyV1` content identity.
    pub policy_id: Id,
    /// Immutable parent facility identity.
    pub facility_id: Id,
    /// Parent generation at which this counted page was created.
    pub counted_generation: u64,
    /// Zero-based canonical page ordinal.
    pub page_ordinal: u32,
    /// Next page ordinal or `NO_NEXT_LP_PAGE` for the tail.
    pub next_page_ordinal: u32,
    /// Number of active leading entries.
    pub entry_count: u8,
    /// Whether the owner/share set is frozen against funding changes.
    pub sealed: bool,
    /// Whether terminal claim amounts have been assigned, including zero claims.
    pub terminal_allocated: bool,
    /// Monotone page revision used by authenticated page-root construction.
    pub revision: u64,
    /// Strictly owner-sorted entries followed by exact empty padding.
    pub entries: [LpEntryV1; LP_ENTRIES_PER_PAGE],
    /// Exact child rent owner.
    pub rent: DeletableRentOwnerV1,
}

impl LpPageV1 {
    /// Validate the page chain coordinate, flags, sorted entries, and padding.
    pub fn validate(&self) -> Result<()> {
        self.policy_id.validate_live()?;
        self.facility_id.validate_live()?;
        if self.page_ordinal >= MAX_LP_PAGES
            || usize::from(self.entry_count) > LP_ENTRIES_PER_PAGE
            || (self.next_page_ordinal != NO_NEXT_LP_PAGE
                && (self.next_page_ordinal >= MAX_LP_PAGES
                    || self.next_page_ordinal != self.page_ordinal + 1
                    || usize::from(self.entry_count) != LP_ENTRIES_PER_PAGE))
            || self.terminal_allocated && !self.sealed
        {
            return Err(Error::InvalidLpPage);
        }
        let count = usize::from(self.entry_count);
        let mut index = 0usize;
        while index < count {
            self.entries[index].validate_live(self.terminal_allocated)?;
            if index != 0 && self.entries[index - 1].owner >= self.entries[index].owner {
                return Err(Error::InvalidLpPage);
            }
            index += 1;
        }
        while index < LP_ENTRIES_PER_PAGE {
            if self.entries[index] != LpEntryV1::EMPTY {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        self.rent.validate()
    }

    /// Join the page's immutable policy identity, page cap, and rent sink to
    /// the exact canonical policy bytes.
    pub fn validate_against_policy(&self, policy: &crate::DealerPolicyV1) -> Result<()> {
        self.validate()?;
        policy.validate()?;
        if self.policy_id != policy.policy_id()?
            || self.page_ordinal >= policy.maximum_lp_pages
            || (self.next_page_ordinal != NO_NEXT_LP_PAGE
                && self.next_page_ordinal >= policy.maximum_lp_pages)
            || self.rent.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Return the exact counted-child edge owned by DealerState.
    pub const fn counted_child(&self) -> CountedDealerChildV1 {
        CountedDealerChildV1 {
            facility_id: self.facility_id,
            kind: DealerChildKindV1::LpPage,
            counted_generation: self.counted_generation,
        }
    }

    /// Canonical mutable-page content identity.
    pub fn page_content_id(&self) -> Result<Id> {
        self.content_id(LP_PAGE_CONTENT_DOMAIN_V1)
    }
}

impl FixedCodec for LpPageV1 {
    const ENCODED_LEN: usize = LP_PAGE_BYTES_V1;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&LP_PAGE_MAGIC_V1, LP_PAGE_VERSION_V1);
        writer.id(self.policy_id);
        writer.id(self.facility_id);
        writer.u64(self.counted_generation);
        writer.u32(self.page_ordinal);
        writer.u32(self.next_page_ordinal);
        writer.u8(self.entry_count);
        writer.bool(self.sealed);
        writer.bool(self.terminal_allocated);
        writer.reserved(1);
        writer.u64(self.revision);
        let mut index = 0usize;
        while index < LP_ENTRIES_PER_PAGE {
            self.entries[index].encode_body(&mut writer);
            index += 1;
        }
        self.rent.encode_body(&mut writer);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.header(&LP_PAGE_MAGIC_V1, LP_PAGE_VERSION_V1)?;
        let policy_id = reader.id();
        let facility_id = reader.id();
        let counted_generation = reader.u64();
        let page_ordinal = reader.u32();
        let next_page_ordinal = reader.u32();
        let entry_count = reader.u8();
        let sealed = reader.bool()?;
        let terminal_allocated = reader.bool()?;
        reader.reserved(1)?;
        let revision = reader.u64();
        let mut entries = [LpEntryV1::EMPTY; LP_ENTRIES_PER_PAGE];
        let mut index = 0usize;
        while index < LP_ENTRIES_PER_PAGE {
            entries[index] = LpEntryV1::decode_body(&mut reader)?;
            index += 1;
        }
        let value = Self {
            policy_id,
            facility_id,
            counted_generation,
            page_ordinal,
            next_page_ordinal,
            entry_count,
            sealed,
            terminal_allocated,
            revision,
            entries,
            rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(LP_ENTRY_BYTES_V1 == 64);
const _: () = assert!(LP_PAGE_BYTES_V1 == 1_208);
const _: () = assert!(LP_PAGE_BYTES_V1 <= crate::MAX_SEMANTIC_BODY_BYTES);
