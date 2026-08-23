// SPDX-License-Identifier: AGPL-3.0-or-later

//! V2 LP ownership pages with terminal claims owned by separate allocations.

use crate::codec::{Reader, Writer, HEADER_BYTES};
use crate::{
    CountedDealerChildV2, DealerChildKindV2, DealerPolicyV1, DealerStateV2,
    DeletableRentOwnerV1, Error, FixedCodec, Id, Result, DELETABLE_RENT_OWNER_BYTES,
    LP_ENTRIES_PER_PAGE, LP_PAGE_CONTENT_DOMAIN_V2, MAX_ATOMS, MAX_LP_PAGES,
    NO_NEXT_LP_PAGE,
};

/// Local semantic magic for V2 LP pages.
pub const LP_PAGE_MAGIC_V2: [u8; 8] = *b"DCLPPGV2";
/// Exact local semantic version.
pub const LP_PAGE_VERSION_V2: u16 = 2;
/// Exact bytes in one V2 LP entry.
pub const LP_ENTRY_BYTES_V2: usize = 48;
/// Exact canonical V2 page bytes.
pub const LP_PAGE_BYTES_V2: usize = HEADER_BYTES
    + (4 * 32)
    + 32
    + (LP_ENTRIES_PER_PAGE * LP_ENTRY_BYTES_V2)
    + DELETABLE_RENT_OWNER_BYTES;

/// Immutable LP ownership/share fact after activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LpEntryV2 {
    /// Exact ordinary Position owner.
    pub owner: Id,
    /// Exact capital-unit shares.
    pub shares: u64,
    /// Irrevocably queued shares.
    pub queued_shares: u64,
}

impl LpEntryV2 {
    /// Canonical inactive padding.
    pub const EMPTY: Self = Self {
        owner: Id::ZERO,
        shares: 0,
        queued_shares: 0,
    };

    fn validate_live(&self) -> Result<()> {
        self.owner.validate_live()?;
        if self.shares == 0 || self.shares > MAX_ATOMS || self.queued_shares > self.shares {
            return Err(Error::InvalidLpPage);
        }
        Ok(())
    }

    fn encode_body(&self, writer: &mut Writer<'_>) {
        writer.id(self.owner);
        writer.u64(self.shares);
        writer.u64(self.queued_shares);
    }

    fn decode_body(reader: &mut Reader<'_>) -> Self {
        Self {
            owner: reader.id(),
            shares: reader.u64(),
            queued_shares: reader.u64(),
        }
    }
}

/// One strictly owner-sorted LP page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LpPageV2 {
    /// Exact Dealer policy.
    pub policy_id: Id,
    /// Immutable facility identity.
    pub facility_id: Id,
    /// Immutable canonical Position V3 purpose-binding identity.
    pub facility_position_binding_id: Id,
    /// Authoritative DealerState account.
    pub dealer_state_account_id: Id,
    /// Parent generation at admission.
    pub counted_generation: u64,
    /// Zero-based page ordinal.
    pub page_ordinal: u32,
    /// Next page ordinal or `NO_NEXT_LP_PAGE`.
    pub next_page_ordinal: u32,
    /// Number of active leading entries.
    pub entry_count: u8,
    /// Whether funding mutations are permanently disabled.
    pub sealed: bool,
    /// Monotone pre-seal revision.
    pub revision: u64,
    /// Strictly sorted active entries and exact empty padding.
    pub entries: [LpEntryV2; LP_ENTRIES_PER_PAGE],
    /// Independently funded page rent.
    pub rent: DeletableRentOwnerV1,
}

impl LpPageV2 {
    /// Validate page coordinates, sorted ownership, padding, and rent.
    pub fn validate(&self) -> Result<()> {
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.dealer_state_account_id,
        ] {
            identity.validate_live()?;
        }
        let count = usize::from(self.entry_count);
        if self.page_ordinal >= MAX_LP_PAGES
            || count == 0
            || count > LP_ENTRIES_PER_PAGE
            || (self.next_page_ordinal != NO_NEXT_LP_PAGE
                && (self.next_page_ordinal != self.page_ordinal + 1
                    || self.next_page_ordinal >= MAX_LP_PAGES
                    || count != LP_ENTRIES_PER_PAGE))
        {
            return Err(Error::InvalidLpPage);
        }
        let mut index = 0usize;
        while index < count {
            self.entries[index].validate_live()?;
            if index != 0 && self.entries[index - 1].owner >= self.entries[index].owner {
                return Err(Error::InvalidLpPage);
            }
            index += 1;
        }
        while index < LP_ENTRIES_PER_PAGE {
            if self.entries[index] != LpEntryV2::EMPTY {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        self.rent.validate()
    }

    /// Join immutable page facts to Policy and State.
    pub fn validate_against(
        &self,
        policy: &DealerPolicyV1,
        state: &DealerStateV2,
        state_account_id: Id,
    ) -> Result<()> {
        self.validate()?;
        state.validate_against_policy(policy)?;
        if self.policy_id != policy.policy_id()?
            || self.facility_id != state.facility_id
            || self.facility_position_binding_id != state.facility_position_binding_id
            || self.dealer_state_account_id != state_account_id
            || self.page_ordinal >= policy.maximum_lp_pages
            || (self.next_page_ordinal != NO_NEXT_LP_PAGE
                && self.next_page_ordinal >= policy.maximum_lp_pages)
            || self.counted_generation > state.generation
            || self.rent.neutral_sink != policy.neutral_sink
        {
            return Err(Error::MismatchedBinding);
        }
        Ok(())
    }

    /// Sum page shares with checked arithmetic.
    pub fn share_totals(&self) -> Result<(u64, u64)> {
        self.validate()?;
        let mut total = 0u64;
        let mut queued = 0u64;
        let mut index = 0usize;
        while index < usize::from(self.entry_count) {
            total = total
                .checked_add(self.entries[index].shares)
                .ok_or(Error::ArithmeticOverflow)?;
            queued = queued
                .checked_add(self.entries[index].queued_shares)
                .ok_or(Error::ArithmeticOverflow)?;
            index += 1;
        }
        Ok((total, queued))
    }

    /// Counted V2 child edge.
    pub const fn counted_child(&self) -> CountedDealerChildV2 {
        CountedDealerChildV2 {
            facility_id: self.facility_id,
            facility_position_binding_id: self.facility_position_binding_id,
            kind: DealerChildKindV2::LpPage,
            counted_generation: self.counted_generation,
        }
    }

    /// Exact semantic page identity.
    pub fn page_content_id(&self) -> Result<Id> {
        self.content_id(LP_PAGE_CONTENT_DOMAIN_V2)
    }
}

impl FixedCodec for LpPageV2 {
    const ENCODED_LEN: usize = LP_PAGE_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.header(&LP_PAGE_MAGIC_V2, LP_PAGE_VERSION_V2);
        for identity in [
            self.policy_id,
            self.facility_id,
            self.facility_position_binding_id,
            self.dealer_state_account_id,
        ] {
            writer.id(identity);
        }
        writer.u64(self.counted_generation);
        writer.u32(self.page_ordinal);
        writer.u32(self.next_page_ordinal);
        writer.u8(self.entry_count);
        writer.bool(self.sealed);
        writer.reserved(6);
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
        reader.header(&LP_PAGE_MAGIC_V2, LP_PAGE_VERSION_V2)?;
        let policy_id = reader.id();
        let facility_id = reader.id();
        let facility_position_binding_id = reader.id();
        let dealer_state_account_id = reader.id();
        let counted_generation = reader.u64();
        let page_ordinal = reader.u32();
        let next_page_ordinal = reader.u32();
        let entry_count = reader.u8();
        let sealed = reader.bool()?;
        reader.reserved(6)?;
        let revision = reader.u64();
        let mut entries = [LpEntryV2::EMPTY; LP_ENTRIES_PER_PAGE];
        let mut index = 0usize;
        while index < LP_ENTRIES_PER_PAGE {
            entries[index] = LpEntryV2::decode_body(&mut reader);
            index += 1;
        }
        let value = Self {
            policy_id,
            facility_id,
            facility_position_binding_id,
            dealer_state_account_id,
            counted_generation,
            page_ordinal,
            next_page_ordinal,
            entry_count,
            sealed,
            revision,
            entries,
            rent: DeletableRentOwnerV1::decode_body(&mut reader),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

const _: () = assert!(LP_ENTRY_BYTES_V2 == 48);
const _: () = assert!(LP_PAGE_BYTES_V2 == 1_020);
const _: () = assert!(LP_PAGE_BYTES_V2 <= crate::MAX_SEMANTIC_BODY_BYTES);
