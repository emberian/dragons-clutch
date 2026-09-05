//! Ordered semantic roles of the canonical Rational representation frame.
//!
//! [`RepresentationFrameSpecV2`] already owned the frame's three WIDTHS. It did
//! not own the frame's ORDER, so every reader of the frame -- the Claims
//! adapter that parses it, the operator that builds it, the AccountProfile that
//! projects it -- carried its own list of what sits at each index. Three
//! private lists agreeing by inspection is the shape a width drifts in: the
//! terminal AccountProfile spent its whole life one account narrower than the
//! frame the same crate specified, and nothing could go red because no two of
//! those lists were ever compared.
//!
//! This module is that comparison, made structural. The three ordered arrays
//! below are the sole author of the order; [`RepresentationFrameSpecV2::coordinate`]
//! reads an index out of them under the spec's own widths, and
//! [`RepresentationCoordinateV2::segment_index`] reads the index back out of a
//! role. A coordinate added to the frame is then a non-exhaustive `match` in
//! every consumer that classifies coordinates, which is a compile error rather
//! than a silent disagreement.

// Every index in this module is a `const fn` walk bounded one line above by the
// width of the very array being indexed, and `<[T]>::get` is not `const`. The
// suppression is the whole file because the alternative -- a `#[expect]` on each
// of five identical walks -- reads as five separate decisions.
#![allow(
    clippy::indexing_slicing,
    reason = "const-evaluated walks bounded by the indexed array's own width"
)]

use crate::rational_request::{
    RATIONAL_ASSET_ACCOUNT_COUNT_V2, RATIONAL_BASE_ACCOUNT_COUNT_V2,
    RATIONAL_TERMINAL_ACCOUNT_COUNT_V2, RepresentationFrameSpecV2,
};

/// The three ordered segments a representation frame is built from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepresentationSegmentV2 {
    /// The fixed prefix every action carries.
    Base,
    /// One repeated active asset row.
    Asset,
    /// The suffix only `RedeemTerminal` carries.
    Terminal,
}

/// One account's semantic role in the canonical Rational representation frame.
///
/// The declaration order of each segment's variants is not the authority; the
/// three `REPRESENTATION_*_COORDINATES_V2` arrays are, and the compile-time
/// round-trip at the bottom of this module holds them to each other.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepresentationCoordinateV2 {
    /// Release-selected caller PDA that signs the child request.
    CallerAuthority,
    /// Caller program whose PDA the caller authority derives from.
    CallerProgram,
    /// Caller program's ProgramData account.
    CallerProgramdata,
    /// Wallet actor named by the request header.
    Actor,
    /// Representation authority PDA derived from the descriptor id.
    RepresentationAuthority,
    /// Finalized representation descriptor record.
    DescriptorRaw,
    /// Representation descriptor staging cursor.
    DescriptorStaging,
    /// Finalized composition exposure record.
    GraphRaw,
    /// Composition exposure staging cursor.
    GraphStaging,
    /// Rent sysvar.
    RentSysvar,
    /// System program.
    SystemProgram,
    /// Representation replay cursor.
    Replay,
    /// Claims liability-basis aggregate.
    ClaimsAggregate,
    /// Activated execution release-set cache.
    ActivationCache,
    /// Claims program.
    ClaimsProgram,
    /// Claims program's ProgramData account.
    ClaimsProgramdata,
    /// Registry program.
    RegistryProgram,
    /// Core Market state.
    CoreMarket,
    /// Core program.
    CoreProgram,
    /// Core program's ProgramData account.
    CoreProgramdata,
    /// Representation receipt mint.
    ReceiptMint,
    /// Actor's receipt token account, present only for the structured actions.
    ActorReceiptAccount,
    /// Token program named by the request header.
    TokenProgram,
    /// Actor's Claims Position, present only for the selected actions.
    ActorClaimsPosition,
    /// Finalized linked Product basis record.
    LinkedBasisRecord,
    /// Linked Product basis staging cursor.
    LinkedBasisStaging,
    /// Finalized Product record.
    ProductRecord,
    /// Product staging cursor.
    ProductStaging,
    /// Finalized ResultDomain record.
    ResultDomainRecord,
    /// ResultDomain staging cursor.
    ResultDomainStaging,
    /// Finalized portfolio record.
    PortfolioRecord,
    /// Portfolio staging cursor.
    PortfolioStaging,

    /// This row's Claims Custody Position.
    AssetPosition,
    /// This row's shard mint.
    AssetShardMint,
    /// Actor's token account for this row's shard mint.
    AssetActorToken,
    /// Structured custody token account for this row's shard mint.
    AssetStructuredToken,

    /// Custody caller PDA the terminal child invokes Custody through.
    TerminalCallerAuthority,
    /// Custody program.
    TerminalCustodyProgram,
    /// Custody program's ProgramData account.
    TerminalCustodyProgramdata,
    /// Resolution certificate authorizing the terminal payout.
    TerminalCertificate,
    /// Resolution program.
    TerminalResolutionProgram,
    /// Resolution program's ProgramData account.
    TerminalResolutionProgramdata,
    /// Finalized Realm record.
    TerminalRealm,
    /// Realm staging cursor.
    TerminalRealmStaging,
    /// Custody replay cursor.
    TerminalCustodyReplay,
    /// Collateral mint the Hoard holds.
    TerminalCollateralMint,
    /// Custody Hoard token account.
    TerminalHoard,
    /// Collateral recipient token account.
    TerminalRecipient,
    /// Custody authority PDA.
    TerminalCustodyAuthority,
    /// Token program the Realm names for collateral movement.
    TerminalTokenProgram,
}

/// Ordered fixed prefix of every representation frame.
pub const REPRESENTATION_BASE_COORDINATES_V2: [RepresentationCoordinateV2;
    RATIONAL_BASE_ACCOUNT_COUNT_V2] = [
    RepresentationCoordinateV2::CallerAuthority,
    RepresentationCoordinateV2::CallerProgram,
    RepresentationCoordinateV2::CallerProgramdata,
    RepresentationCoordinateV2::Actor,
    RepresentationCoordinateV2::RepresentationAuthority,
    RepresentationCoordinateV2::DescriptorRaw,
    RepresentationCoordinateV2::DescriptorStaging,
    RepresentationCoordinateV2::GraphRaw,
    RepresentationCoordinateV2::GraphStaging,
    RepresentationCoordinateV2::RentSysvar,
    RepresentationCoordinateV2::SystemProgram,
    RepresentationCoordinateV2::Replay,
    RepresentationCoordinateV2::ClaimsAggregate,
    RepresentationCoordinateV2::ActivationCache,
    RepresentationCoordinateV2::ClaimsProgram,
    RepresentationCoordinateV2::ClaimsProgramdata,
    RepresentationCoordinateV2::RegistryProgram,
    RepresentationCoordinateV2::CoreMarket,
    RepresentationCoordinateV2::CoreProgram,
    RepresentationCoordinateV2::CoreProgramdata,
    RepresentationCoordinateV2::ReceiptMint,
    RepresentationCoordinateV2::ActorReceiptAccount,
    RepresentationCoordinateV2::TokenProgram,
    RepresentationCoordinateV2::ActorClaimsPosition,
    RepresentationCoordinateV2::LinkedBasisRecord,
    RepresentationCoordinateV2::LinkedBasisStaging,
    RepresentationCoordinateV2::ProductRecord,
    RepresentationCoordinateV2::ProductStaging,
    RepresentationCoordinateV2::ResultDomainRecord,
    RepresentationCoordinateV2::ResultDomainStaging,
    RepresentationCoordinateV2::PortfolioRecord,
    RepresentationCoordinateV2::PortfolioStaging,
];

/// Ordered accounts of one active asset row.
pub const REPRESENTATION_ASSET_COORDINATES_V2: [RepresentationCoordinateV2;
    RATIONAL_ASSET_ACCOUNT_COUNT_V2] = [
    RepresentationCoordinateV2::AssetPosition,
    RepresentationCoordinateV2::AssetShardMint,
    RepresentationCoordinateV2::AssetActorToken,
    RepresentationCoordinateV2::AssetStructuredToken,
];

/// Ordered suffix appended only for terminal redemption.
pub const REPRESENTATION_TERMINAL_COORDINATES_V2: [RepresentationCoordinateV2;
    RATIONAL_TERMINAL_ACCOUNT_COUNT_V2] = [
    RepresentationCoordinateV2::TerminalCallerAuthority,
    RepresentationCoordinateV2::TerminalCustodyProgram,
    RepresentationCoordinateV2::TerminalCustodyProgramdata,
    RepresentationCoordinateV2::TerminalCertificate,
    RepresentationCoordinateV2::TerminalResolutionProgram,
    RepresentationCoordinateV2::TerminalResolutionProgramdata,
    RepresentationCoordinateV2::TerminalRealm,
    RepresentationCoordinateV2::TerminalRealmStaging,
    RepresentationCoordinateV2::TerminalCustodyReplay,
    RepresentationCoordinateV2::TerminalCollateralMint,
    RepresentationCoordinateV2::TerminalHoard,
    RepresentationCoordinateV2::TerminalRecipient,
    RepresentationCoordinateV2::TerminalCustodyAuthority,
    RepresentationCoordinateV2::TerminalTokenProgram,
];

impl RepresentationCoordinateV2 {
    /// Which of the three ordered segments this coordinate belongs to.
    pub const fn segment(self) -> RepresentationSegmentV2 {
        match self {
            Self::CallerAuthority
            | Self::CallerProgram
            | Self::CallerProgramdata
            | Self::Actor
            | Self::RepresentationAuthority
            | Self::DescriptorRaw
            | Self::DescriptorStaging
            | Self::GraphRaw
            | Self::GraphStaging
            | Self::RentSysvar
            | Self::SystemProgram
            | Self::Replay
            | Self::ClaimsAggregate
            | Self::ActivationCache
            | Self::ClaimsProgram
            | Self::ClaimsProgramdata
            | Self::RegistryProgram
            | Self::CoreMarket
            | Self::CoreProgram
            | Self::CoreProgramdata
            | Self::ReceiptMint
            | Self::ActorReceiptAccount
            | Self::TokenProgram
            | Self::ActorClaimsPosition
            | Self::LinkedBasisRecord
            | Self::LinkedBasisStaging
            | Self::ProductRecord
            | Self::ProductStaging
            | Self::ResultDomainRecord
            | Self::ResultDomainStaging
            | Self::PortfolioRecord
            | Self::PortfolioStaging => RepresentationSegmentV2::Base,
            Self::AssetPosition
            | Self::AssetShardMint
            | Self::AssetActorToken
            | Self::AssetStructuredToken => RepresentationSegmentV2::Asset,
            Self::TerminalCallerAuthority
            | Self::TerminalCustodyProgram
            | Self::TerminalCustodyProgramdata
            | Self::TerminalCertificate
            | Self::TerminalResolutionProgram
            | Self::TerminalResolutionProgramdata
            | Self::TerminalRealm
            | Self::TerminalRealmStaging
            | Self::TerminalCustodyReplay
            | Self::TerminalCollateralMint
            | Self::TerminalHoard
            | Self::TerminalRecipient
            | Self::TerminalCustodyAuthority
            | Self::TerminalTokenProgram => RepresentationSegmentV2::Terminal,
        }
    }

    /// This coordinate's exact index within its own ordered segment.
    ///
    /// Total by construction: the compile-time round-trip below proves every
    /// variant appears exactly once in the array its [`Self::segment`] names, so
    /// the search never falls through. A variant that stops appearing turns the
    /// `assert!` red at compile time rather than returning a wrong index.
    pub const fn segment_index(self) -> usize {
        let segment = match self.segment() {
            RepresentationSegmentV2::Base => REPRESENTATION_BASE_COORDINATES_V2.as_slice(),
            RepresentationSegmentV2::Asset => REPRESENTATION_ASSET_COORDINATES_V2.as_slice(),
            RepresentationSegmentV2::Terminal => REPRESENTATION_TERMINAL_COORDINATES_V2.as_slice(),
        };
        let mut index = 0;
        while index < segment.len() {
            let candidate = segment[index];
            if candidate.discriminant() == self.discriminant() {
                return index;
            }
            index = index.saturating_add(1);
        }
        // Unreachable under the round-trip assertion below, and a wrong index is
        // worse than an out-of-range one: every caller compares it to a width.
        usize::MAX
    }

    /// Opaque identity used only to compare two coordinates in a `const fn`.
    ///
    /// `PartialEq` is not `const`, and a hand-written match returning a number
    /// per variant would make this function a SECOND author of the frame order.
    /// The compiler's own discriminant is derived from the declaration and is
    /// never read as a position.
    const fn discriminant(self) -> u8 {
        // Fieldless enum, fewer variants than `u8::MAX` (asserted below), so
        // nothing truncates. `allow` rather than `expect` because which of the
        // two truncation lints fires on an enum cast is a clippy detail.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_enum_truncation,
            reason = "fieldless enum narrower than u8::MAX, asserted below"
        )]
        let value = self as u8;
        value
    }
}

impl RepresentationFrameSpecV2 {
    /// Ordered role at `index` of the frame this spec describes for a request
    /// with `assets` active rows and, when `terminal`, the terminal suffix.
    ///
    /// `None` past the frame's end, which is exactly
    /// [`RepresentationFrameSpecV2::account_count`] for the same shape.
    pub const fn coordinate(
        self,
        index: usize,
        assets: usize,
        terminal: bool,
    ) -> Option<RepresentationCoordinateV2> {
        if index < self.fixed_accounts() {
            return Some(REPRESENTATION_BASE_COORDINATES_V2[index]);
        }
        let Some(after_base) = index.checked_sub(self.fixed_accounts()) else {
            return None;
        };
        let Some(asset_span) = assets.checked_mul(self.asset_account_stride()) else {
            return None;
        };
        if self.asset_account_stride() == 0 {
            return None;
        }
        if after_base < asset_span {
            let offset = after_base % self.asset_account_stride();
            return Some(REPRESENTATION_ASSET_COORDINATES_V2[offset]);
        }
        if !terminal {
            return None;
        }
        let Some(after_assets) = after_base.checked_sub(asset_span) else {
            return None;
        };
        if after_assets < self.terminal_account_suffix() {
            return Some(REPRESENTATION_TERMINAL_COORDINATES_V2[after_assets]);
        }
        None
    }
}

// EVERY COORDINATE APPEARS EXACTLY ONCE, AT THE INDEX IT REPORTS. This is what
// makes `segment_index` total and what makes the three arrays -- rather than the
// declaration order of the enum -- the frame's single author.
const _: () = {
    let mut index = 0;
    while index < RATIONAL_BASE_ACCOUNT_COUNT_V2 {
        assert!(
            REPRESENTATION_BASE_COORDINATES_V2[index].segment_index() == index,
            "base coordinate order must round-trip"
        );
        index += 1;
    }
    let mut index = 0;
    while index < RATIONAL_ASSET_ACCOUNT_COUNT_V2 {
        assert!(
            REPRESENTATION_ASSET_COORDINATES_V2[index].segment_index() == index,
            "asset row coordinate order must round-trip"
        );
        index += 1;
    }
    let mut index = 0;
    while index < RATIONAL_TERMINAL_ACCOUNT_COUNT_V2 {
        assert!(
            REPRESENTATION_TERMINAL_COORDINATES_V2[index].segment_index() == index,
            "terminal suffix coordinate order must round-trip"
        );
        index += 1;
    }
    assert!(
        RATIONAL_BASE_ACCOUNT_COUNT_V2
            + RATIONAL_ASSET_ACCOUNT_COUNT_V2
            + RATIONAL_TERMINAL_ACCOUNT_COUNT_V2
            < u8::MAX as usize,
        "the frame must stay narrower than the discriminant width `segment_index` compares"
    );
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rational_request::{REPRESENTATION_FRAME_SPEC_V2, RepresentationCoordinateV2 as Coordinate};

    /// The single-asset terminal frame the Hot route builds, walked end to end.
    #[test]
    fn single_asset_terminal_frame_walks_fifty_named_coordinates() {
        let spec = REPRESENTATION_FRAME_SPEC_V2;
        let width =
            spec.fixed_accounts() + spec.asset_account_stride() + spec.terminal_account_suffix();
        assert_eq!(width, 50);
        assert_eq!(
            spec.coordinate(0, 1, true),
            Some(Coordinate::CallerAuthority)
        );
        assert_eq!(
            spec.coordinate(spec.fixed_accounts(), 1, true),
            Some(Coordinate::AssetPosition)
        );
        assert_eq!(
            spec.coordinate(spec.fixed_accounts() + spec.asset_account_stride(), 1, true),
            Some(Coordinate::TerminalCallerAuthority)
        );
        assert_eq!(
            spec.coordinate(width - 1, 1, true),
            Some(Coordinate::TerminalTokenProgram)
        );
        assert_eq!(spec.coordinate(width, 1, true), None);
        for index in 0..width {
            assert!(spec.coordinate(index, 1, true).is_some(), "index {index}");
        }
    }

    /// A non-terminal frame stops where the spec says it stops.
    #[test]
    fn open_frame_ends_after_its_asset_rows() {
        let spec = REPRESENTATION_FRAME_SPEC_V2;
        let width = spec.fixed_accounts() + spec.asset_account_stride();
        assert_eq!(
            spec.coordinate(width - 1, 1, false),
            Some(Coordinate::AssetStructuredToken)
        );
        assert_eq!(spec.coordinate(width, 1, false), None);
    }

    /// Two asset rows repeat the same four roles at the same stride.
    #[test]
    fn asset_rows_repeat_the_row_declaration() {
        let spec = REPRESENTATION_FRAME_SPEC_V2;
        let base = spec.fixed_accounts();
        for row in 0..2 {
            for (offset, expected) in REPRESENTATION_ASSET_COORDINATES_V2.iter().enumerate() {
                assert_eq!(
                    spec.coordinate(base + row * spec.asset_account_stride() + offset, 2, true),
                    Some(*expected),
                );
            }
        }
    }
}
