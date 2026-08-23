// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::codec::{Reader, Writer};
use crate::{add, Error, Id, Result};

/// Exact bytes in one deletable account's rent ownership record.
pub const DELETABLE_RENT_OWNER_BYTES: usize = 80;
/// Exact bytes in the root's shrink-to-tombstone rent ownership record.
pub const ROOT_RENT_OWNER_BYTES: usize = 88;

/// Exact rent ownership for a child deleted at close.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeletableRentOwnerV1 {
    /// Payer and sole recipient of refundable rent principal.
    pub payer: Id,
    /// Immutable Realm/Profile sink receiving prefunds and later surplus.
    pub neutral_sink: Id,
    /// Principal supplied without any hostile-prefund discount.
    pub refundable_principal: u64,
    /// Prefund observed at creation and permanently routed to the neutral sink.
    pub donation_floor: u64,
}

impl DeletableRentOwnerV1 {
    /// Validate a live payer, positive principal, and bounded total lamports.
    pub fn validate(&self) -> Result<()> {
        self.payer.validate_live()?;
        self.neutral_sink.validate_live()?;
        if self.payer == self.neutral_sink || self.refundable_principal == 0 {
            return Err(Error::InvalidParameter);
        }
        add(self.refundable_principal, self.donation_floor)?;
        Ok(())
    }

    pub(crate) fn encode_body(&self, writer: &mut Writer<'_>) {
        writer.id(self.payer);
        writer.id(self.neutral_sink);
        writer.u64(self.refundable_principal);
        writer.u64(self.donation_floor);
    }

    pub(crate) fn decode_body(reader: &mut Reader<'_>) -> Self {
        Self {
            payer: reader.id(),
            neutral_sink: reader.id(),
            refundable_principal: reader.u64(),
            donation_floor: reader.u64(),
        }
    }
}

/// Exact root rent split for eventual shrink to a permanent tombstone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootRentOwnerV1 {
    /// Payer and sole recipient of refundable live-state principal.
    pub payer: Id,
    /// Immutable Realm/Profile sink receiving prefunds and later surplus.
    pub neutral_sink: Id,
    /// Live-state rent delta returned only to `payer`.
    pub refundable_live_principal: u64,
    /// Independently prepaid principal retained by the tombstone.
    pub permanent_tombstone_principal: u64,
    /// Creation-time hostile prefund routed to the neutral sink.
    pub donation_floor: u64,
}

impl RootRentOwnerV1 {
    /// Validate both principal compartments and their exact checked sum.
    pub fn validate(&self) -> Result<()> {
        self.payer.validate_live()?;
        self.neutral_sink.validate_live()?;
        if self.payer == self.neutral_sink
            || self.refundable_live_principal == 0
            || self.permanent_tombstone_principal == 0
        {
            return Err(Error::InvalidParameter);
        }
        add(
            add(
                self.refundable_live_principal,
                self.permanent_tombstone_principal,
            )?,
            self.donation_floor,
        )?;
        Ok(())
    }

    pub(crate) fn encode_body(&self, writer: &mut Writer<'_>) {
        writer.id(self.payer);
        writer.id(self.neutral_sink);
        writer.u64(self.refundable_live_principal);
        writer.u64(self.permanent_tombstone_principal);
        writer.u64(self.donation_floor);
    }

    pub(crate) fn decode_body(reader: &mut Reader<'_>) -> Self {
        Self {
            payer: reader.id(),
            neutral_sink: reader.id(),
            refundable_live_principal: reader.u64(),
            permanent_tombstone_principal: reader.u64(),
            donation_floor: reader.u64(),
        }
    }
}

const _: () = assert!(DELETABLE_RENT_OWNER_BYTES == (2 * 32) + 8 + 8);
const _: () = assert!(ROOT_RENT_OWNER_BYTES == (2 * 32) + 8 + 8 + 8);
