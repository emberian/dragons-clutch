//! Canonical non-self-referential Rational resource identities.

use crate::{Error, Result, require_nonzero};

/// Claims PDA seed for one graph/Market/release Structured receipt Mint.
pub const RATIONAL_RECEIPT_MINT_SEED_V2: &[u8] = b"dclutch:rational-receipt:v2";

/// Independent immutable coordinates for the canonical Structured receipt
/// Mint PDA.
///
/// The finalized descriptor persists the resulting Mint address, so neither
/// the descriptor digest nor that persisted address may participate in this
/// derivation. The graph digest binds the exact graph, coefficient vector,
/// root, width, and denominator; Market and release prevent cross-context
/// reuse of the same graph asset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalReceiptMintSeedsV2 {
    graph_digest: [u8; 32],
    market: [u8; 32],
    release_set: [u8; 32],
}

impl RationalReceiptMintSeedsV2 {
    /// Construct one canonical, nonzero, nonaliasing coordinate set.
    pub fn new(graph_digest: [u8; 32], market: [u8; 32], release_set: [u8; 32]) -> Result<Self> {
        let graph_digest = require_nonzero(graph_digest)?;
        let market = require_nonzero(market)?;
        let release_set = require_nonzero(release_set)?;
        if graph_digest == market || graph_digest == release_set || market == release_set {
            return Err(Error::AccountAlias);
        }
        Ok(Self {
            graph_digest,
            market,
            release_set,
        })
    }

    /// Borrow the sole exact receipt-Mint PDA seed order, excluding bump.
    pub fn as_slices(&self) -> [&[u8]; 4] {
        [
            RATIONAL_RECEIPT_MINT_SEED_V2,
            &self.graph_digest,
            &self.market,
            &self.release_set,
        ]
    }

    /// Join an adapter-derived PDA to the address persisted by the finalized
    /// descriptor.
    ///
    /// PDA derivation itself remains in the small SVM adapter boundary. This
    /// method closes alternate-address and semantic-coordinate aliasing after
    /// that derivation.
    pub fn authenticate_address(
        self,
        derived_address: [u8; 32],
        persisted_address: [u8; 32],
    ) -> Result<[u8; 32]> {
        let derived_address = require_nonzero(derived_address)?;
        let persisted_address = require_nonzero(persisted_address)?;
        if derived_address != persisted_address {
            return Err(Error::ProjectionMismatch);
        }
        if derived_address == self.graph_digest
            || derived_address == self.market
            || derived_address == self.release_set
        {
            return Err(Error::AccountAlias);
        }
        Ok(derived_address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    #[test]
    fn exact_independent_seed_order_and_address_join_are_closed() {
        let seeds = RationalReceiptMintSeedsV2::new(id(1), id(2), id(3)).expect("seeds");
        assert_eq!(
            seeds.as_slices(),
            [
                RATIONAL_RECEIPT_MINT_SEED_V2,
                id(1).as_slice(),
                id(2).as_slice(),
                id(3).as_slice(),
            ]
        );
        assert_eq!(seeds.authenticate_address(id(4), id(4)), Ok(id(4)));
        assert_eq!(
            seeds.authenticate_address(id(4), id(5)),
            Err(Error::ProjectionMismatch)
        );
    }

    #[test]
    fn zero_alias_and_derived_coordinate_aliases_refuse() {
        for hostile in [
            RationalReceiptMintSeedsV2::new([0; 32], id(2), id(3)),
            RationalReceiptMintSeedsV2::new(id(1), [0; 32], id(3)),
            RationalReceiptMintSeedsV2::new(id(1), id(2), [0; 32]),
            RationalReceiptMintSeedsV2::new(id(1), id(1), id(3)),
            RationalReceiptMintSeedsV2::new(id(1), id(2), id(1)),
            RationalReceiptMintSeedsV2::new(id(1), id(2), id(2)),
        ] {
            assert!(hostile.is_err());
        }
        let seeds = RationalReceiptMintSeedsV2::new(id(1), id(2), id(3)).expect("seeds");
        for alias in [id(1), id(2), id(3)] {
            assert_eq!(
                seeds.authenticate_address(alias, alias),
                Err(Error::AccountAlias)
            );
        }
    }

    #[test]
    fn same_width_graph_or_coefficient_digest_and_alternate_mint_substitute() {
        let canonical = RationalReceiptMintSeedsV2::new(id(1), id(2), id(3)).expect("canonical");
        let graph_substitution =
            RationalReceiptMintSeedsV2::new(id(9), id(2), id(3)).expect("graph substitution");
        let coefficient_substitution =
            RationalReceiptMintSeedsV2::new(id(10), id(2), id(3)).expect("coefficient digest");
        assert_ne!(canonical.as_slices(), graph_substitution.as_slices());
        assert_ne!(canonical.as_slices(), coefficient_substitution.as_slices());
        assert_eq!(
            canonical.authenticate_address(id(4), id(11)),
            Err(Error::ProjectionMismatch)
        );
    }
}
