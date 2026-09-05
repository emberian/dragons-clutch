//! Canonical non-self-referential Structured V2 resource identities.
//!
//! Three physical resources are derived rather than named: the Structured root,
//! the receipt Mint, and one shard custody Token account per backed
//! representation coordinate.
//!
//! # These derivations have no on-chain referent, and that is decided
//!
//! Decision 0011 §3b (2026-08-27) took Option A: Structured reaches the chain
//! through the **Rational** child ABI, so every physical resource in the live
//! route is keyed by the Rational `descriptor_id` rather than by anything in
//! this module.  What that costs is exactly this module's claims, and the list
//! is longer than §3a predicted:
//!
//! | this module derives | what actually executes | keyed by |
//! |---|---|---|
//! | root (`STRUCTURED_ROOT_PDA_SEED_V2`, `terms_id`) | `(RATIONAL_REPLAY_SEED_V2, descriptor_id, actor)` | descriptor **and actor** |
//! | receipt Mint (`STRUCTURED_RECEIPT_MINT_PDA_SEED_V2`) | the address the Rational descriptor persists, whose Mint **and** permissioned-burn authorities are `(RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, descriptor_id)` | descriptor |
//! | shard custody (`STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2`, `terms_id`, shard Mint) | `(RATIONAL_STRUCTURED_CUSTODY_SEED_V2, descriptor_id, outcome_le)` | descriptor **and outcome index** |
//!
//! Three consequences are worth stating rather than leaving to be rediscovered:
//!
//! - **The root is not the receipt Mint authority.**  The Rational
//!   representation authority is, in two Token-2022 roles at once: Mint
//!   authority for `MintReceipt` and permissioned-burn authority for
//!   `BurnReceipt`.  Founding must configure both.
//! - **Replay is not a property of the node.**  It is one record per
//!   `(representation, actor)` pair.  The paragraph below that says otherwise
//!   describes this module's model, not the chain's.
//! - **Custody is keyed by the outcome index, not by the shard Mint.**  The
//!   self-authenticating property claimed below is not lost, because Rational
//!   derives the shard Mint from `(descriptor_id, outcome_le)` too -- the
//!   mint-to-custody pairing is still a derivation, just anchored at the index.
//!
//! `terms_id` therefore names no account.  It survives as the host-side
//! identity of a Structured terms record whose economic content -- market,
//! release set, Token program, receipt Mint, graph identity, width,
//! denominator, coefficients -- is carried on chain by the Rational descriptor
//! preimage, and whose remaining coordinates (Product record, result domain,
//! Token behavior, shard terms, shard exposure) are bound by the Rational
//! product admission (`rational_product_v3.rs:154-166`) rather than by a seed.
//!
//! This module is retained deliberately, for the same reason `hot_v2.rs` is
//! (decision 0011 §5): it is a host-side authority on the exact seed ORDER and
//! the anti-aliasing rule, and it is the record of what Structured's own
//! physics would have been.  It is not the chain's.
//!
//! This module deliberately does not derive an address:
//! `find_program_address` needs the owning program id and a curve check, both
//! of which belong to the small SVM adapter boundary.  Every derivation this
//! module describes is joined back to an observed or persisted address through
//! `authenticate_address`, so an adapter that derives with the wrong seeds
//! cannot pass its own join.
//!
//! # Why the root may be keyed by the terms identity and the Mint may not
//!
//! `terms_id` is the finalized content identity of the whole immutable terms
//! record: Market, Product record, result domain, release set, Token program,
//! Token behavior, shard terms, shard exposure, receipt Mint, graph identity,
//! representation width, denominator, and every coefficient.  Naming any of
//! those a second time in a seed list would be a second copy of a fact the
//! terms already own.
//!
//! The terms PERSIST the receipt Mint address, so `terms_id` cannot key the
//! Mint: the Mint address would have to be known before the bytes that commit
//! it are hashed.  The Mint is therefore keyed by
//! [`structured_receipt_mint_preimage_v2`] — the exact terms bytes with the
//! receipt-Mint field REMOVED, under its own domain — which binds the
//! coefficients, the width, the denominator and every identity except the one
//! being derived.  Excising is used rather than zeroing so that a terms record
//! whose Mint field were genuinely zero could not collide with one whose Mint
//! field were excised.

use crate::structured_kernel::{
    STRUCTURED_TERMS_COEFFICIENT_BYTES_V2, STRUCTURED_TERMS_HEADER_BYTES_V2,
    STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2,
};

/// Claims PDA seed for one Structured V2 root.
pub const STRUCTURED_ROOT_PDA_SEED_V2: &[u8] = b"dclutch:structured-root:v2";

/// Claims PDA seed for one Structured V2 receipt Mint.
///
/// `structured` is abbreviated for the same reason
/// `PROJECTED_CUSTODY_CALLER_PDA_DOMAIN_V1` in `dclutch-custody` is:
/// a PDA seed may be at most [`MAX_PDA_SEED_BYTES`] bytes, and the
/// unabbreviated `dclutch:structured-receipt-mint:v2` is thirty-four. That
/// spelling names a seed ORDER no adapter could ever execute --
/// `find_program_address` refuses every bump for an over-long seed, so the
/// receipt Mint it describes had no derivable address at all. The static
/// assertion below is what keeps that from being written again silently.
pub const STRUCTURED_RECEIPT_MINT_PDA_SEED_V2: &[u8] = b"dclutch:struct-receipt-mint:v2";

/// Claims PDA seed for one Structured V2 shard custody Token account.
///
/// Abbreviated for the same reason as the receipt Mint seed above: the
/// unabbreviated `dclutch:structured-shard-custody:v2` is thirty-five bytes and
/// could never derive an address.
pub const STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2: &[u8] = b"dclutch:struct-shard-custody:v2";

/// Maximum bytes in one Solana program-derived-address seed.
pub const MAX_PDA_SEED_BYTES: usize = 32;

// A seed domain longer than 32 bytes can never derive an address, so a module
// whose stated job is to be the authority on the exact seed ORDER must not be
// able to publish an order that no adapter could execute. These three are the
// module's whole seed surface.
const _: () = assert!(
    STRUCTURED_ROOT_PDA_SEED_V2.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed domain longer than 32 bytes can never derive an address"
);
const _: () = assert!(
    STRUCTURED_RECEIPT_MINT_PDA_SEED_V2.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed domain longer than 32 bytes can never derive an address"
);
const _: () = assert!(
    STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2.len() <= MAX_PDA_SEED_BYTES,
    "a PDA seed domain longer than 32 bytes can never derive an address"
);

/// Domain separating the excised-terms preimage from the terms bytes themselves.
///
/// This one is hashed rather than seeded, so it is deliberately NOT length-bound.
pub const STRUCTURED_RECEIPT_MINT_PREIMAGE_DOMAIN_V2: &[u8] =
    b"dclutch/structured-receipt-mint-preimage/v2";

/// Stable Structured V2 derivation refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredSeedErrorV2 {
    /// A seed coordinate was the zero identity.
    ZeroIdentity,
    /// Two independent seed coordinates, or a coordinate and its own derived
    /// address, were the same value.
    AccountAlias,
    /// A derived address disagreed with the persisted or observed address.
    AddressMismatch,
    /// A terms slice or preimage buffer had a noncanonical width.
    InvalidLength,
}

/// Result alias for Structured V2 derivations.
pub type Result<T> = core::result::Result<T, StructuredSeedErrorV2>;

fn require_nonzero(value: [u8; 32]) -> Result<[u8; 32]> {
    if value == [0; 32] {
        return Err(StructuredSeedErrorV2::ZeroIdentity);
    }
    Ok(value)
}

fn join(derived: [u8; 32], expected: [u8; 32], coordinates: &[[u8; 32]]) -> Result<[u8; 32]> {
    let derived = require_nonzero(derived)?;
    let expected = require_nonzero(expected)?;
    if derived != expected {
        return Err(StructuredSeedErrorV2::AddressMismatch);
    }
    if coordinates.contains(&derived) {
        return Err(StructuredSeedErrorV2::AccountAlias);
    }
    Ok(derived)
}

/// Exact seed coordinates for the sole Structured V2 root of one terms record.
///
/// The root is one per finalized terms record and carries no owner: replay is a
/// property of the node, not of an actor.
///
/// **That last sentence describes this module's model and not the executing
/// chain.**  Under decision 0011 §3b the live replay record is
/// `(RATIONAL_REPLAY_SEED_V2, descriptor_id, actor)` -- one per
/// `(representation, actor)` pair.  See the module documentation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredRootSeedsV2 {
    terms_id: [u8; 32],
}

impl StructuredRootSeedsV2 {
    /// Construct one canonical nonzero coordinate set.
    pub fn new(terms_id: [u8; 32]) -> Result<Self> {
        Ok(Self {
            terms_id: require_nonzero(terms_id)?,
        })
    }

    /// Borrow the sole exact root PDA seed order, excluding bump.
    pub fn as_slices(&self) -> [&[u8]; 2] {
        [STRUCTURED_ROOT_PDA_SEED_V2, &self.terms_id]
    }

    /// Join an adapter-derived PDA to the observed root account address.
    pub fn authenticate_address(
        self,
        derived_address: [u8; 32],
        observed_address: [u8; 32],
    ) -> Result<[u8; 32]> {
        join(derived_address, observed_address, &[self.terms_id])
    }
}

/// Exact seed coordinates for the canonical Structured V2 receipt Mint.
///
/// `terms_preimage_id` is the digest of [`structured_receipt_mint_preimage_v2`]
/// and is the only coordinate that binds the coefficients.  Market and release
/// set are named independently so that the same excised terms cannot be reused
/// across contexts even if a future terms schema stopped carrying one of them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredReceiptMintSeedsV2 {
    terms_preimage_id: [u8; 32],
    market: [u8; 32],
    release_set: [u8; 32],
}

impl StructuredReceiptMintSeedsV2 {
    /// Construct one canonical, nonzero, nonaliasing coordinate set.
    pub fn new(
        terms_preimage_id: [u8; 32],
        market: [u8; 32],
        release_set: [u8; 32],
    ) -> Result<Self> {
        let terms_preimage_id = require_nonzero(terms_preimage_id)?;
        let market = require_nonzero(market)?;
        let release_set = require_nonzero(release_set)?;
        if terms_preimage_id == market || terms_preimage_id == release_set || market == release_set
        {
            return Err(StructuredSeedErrorV2::AccountAlias);
        }
        Ok(Self {
            terms_preimage_id,
            market,
            release_set,
        })
    }

    /// Borrow the sole exact receipt-Mint PDA seed order, excluding bump.
    pub fn as_slices(&self) -> [&[u8]; 4] {
        [
            STRUCTURED_RECEIPT_MINT_PDA_SEED_V2,
            &self.terms_preimage_id,
            &self.market,
            &self.release_set,
        ]
    }

    /// Join an adapter-derived PDA to the address the finalized terms persist.
    pub fn authenticate_address(
        self,
        derived_address: [u8; 32],
        persisted_address: [u8; 32],
    ) -> Result<[u8; 32]> {
        join(
            derived_address,
            persisted_address,
            &[self.terms_preimage_id, self.market, self.release_set],
        )
    }
}

/// Exact seed coordinates for one Structured V2 shard custody Token account.
///
/// Custody is keyed by the shard MINT rather than by the representation
/// coordinate, so the mint-to-custody binding is a derivation instead of an
/// index lookup.  The exact claim-shard terms already refuse duplicate shard
/// Mints, so the two keyings admit exactly the same set and only this one is
/// self-authenticating.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredShardCustodySeedsV2 {
    terms_id: [u8; 32],
    shard_mint: [u8; 32],
}

impl StructuredShardCustodySeedsV2 {
    /// Construct one canonical, nonzero, nonaliasing coordinate set.
    pub fn new(terms_id: [u8; 32], shard_mint: [u8; 32]) -> Result<Self> {
        let terms_id = require_nonzero(terms_id)?;
        let shard_mint = require_nonzero(shard_mint)?;
        if terms_id == shard_mint {
            return Err(StructuredSeedErrorV2::AccountAlias);
        }
        Ok(Self {
            terms_id,
            shard_mint,
        })
    }

    /// Borrow the sole exact shard-custody PDA seed order, excluding bump.
    pub fn as_slices(&self) -> [&[u8]; 3] {
        [
            STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2,
            &self.terms_id,
            &self.shard_mint,
        ]
    }

    /// Join an adapter-derived PDA to the observed custody account address.
    pub fn authenticate_address(
        self,
        derived_address: [u8; 32],
        observed_address: [u8; 32],
    ) -> Result<[u8; 32]> {
        join(
            derived_address,
            observed_address,
            &[self.terms_id, self.shard_mint],
        )
    }
}

/// Exact byte width of the excised-terms receipt-Mint preimage for one width.
pub fn structured_receipt_mint_preimage_bytes_v2(representation_width: u32) -> Result<usize> {
    let terms = structured_terms_bytes_for_width_v2(representation_width)?;
    STRUCTURED_RECEIPT_MINT_PREIMAGE_DOMAIN_V2
        .len()
        .checked_add(terms)
        .and_then(|total| total.checked_sub(32))
        .ok_or(StructuredSeedErrorV2::InvalidLength)
}

/// Exact terms width for one representation width.
pub fn structured_terms_bytes_for_width_v2(representation_width: u32) -> Result<usize> {
    usize::try_from(representation_width)
        .ok()
        .and_then(|width| width.checked_mul(STRUCTURED_TERMS_COEFFICIENT_BYTES_V2))
        .and_then(|coefficients| coefficients.checked_add(STRUCTURED_TERMS_HEADER_BYTES_V2))
        .ok_or(StructuredSeedErrorV2::InvalidLength)
}

/// Write the exact receipt-Mint preimage: the domain, then the terms bytes with
/// the persisted receipt-Mint field excised.
///
/// `terms_bytes` must be exactly `header + 8 * representation_width` long, so a
/// truncated or padded terms record cannot produce a preimage at all.
pub fn structured_receipt_mint_preimage_v2(
    terms_bytes: &[u8],
    representation_width: u32,
    output: &mut [u8],
) -> Result<()> {
    if terms_bytes.len() != structured_terms_bytes_for_width_v2(representation_width)?
        || output.len() != structured_receipt_mint_preimage_bytes_v2(representation_width)?
    {
        return Err(StructuredSeedErrorV2::InvalidLength);
    }
    let mint_end = STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2
        .checked_add(32)
        .ok_or(StructuredSeedErrorV2::InvalidLength)?;
    let head = terms_bytes
        .get(..STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2)
        .ok_or(StructuredSeedErrorV2::InvalidLength)?;
    let tail = terms_bytes
        .get(mint_end..)
        .ok_or(StructuredSeedErrorV2::InvalidLength)?;
    let domain = STRUCTURED_RECEIPT_MINT_PREIMAGE_DOMAIN_V2;
    let head_end = domain
        .len()
        .checked_add(head.len())
        .ok_or(StructuredSeedErrorV2::InvalidLength)?;
    output
        .get_mut(..domain.len())
        .ok_or(StructuredSeedErrorV2::InvalidLength)?
        .copy_from_slice(domain);
    output
        .get_mut(domain.len()..head_end)
        .ok_or(StructuredSeedErrorV2::InvalidLength)?
        .copy_from_slice(head);
    output
        .get_mut(head_end..)
        .ok_or(StructuredSeedErrorV2::InvalidLength)?
        .copy_from_slice(tail);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    /// Every seed-order assertion below compares `as_slices()` against the very
    /// constant it is built from, so all of them hold for any spelling -- they
    /// pin the ORDER and are blind to the bytes. These two pin the bytes.
    ///
    /// The length bound is the one that matters: a domain over
    /// [`MAX_PDA_SEED_BYTES`] makes `find_program_address` refuse every bump, so
    /// the resource it names has no derivable address and the seed order this
    /// module publishes could not be executed by any adapter. The
    /// `structured-receipt-mint` and `structured-shard-custody` spellings were
    /// thirty-four and thirty-five bytes and did exactly that.
    #[test]
    fn every_seed_domain_is_short_enough_to_actually_derive() {
        for domain in [
            STRUCTURED_ROOT_PDA_SEED_V2,
            STRUCTURED_RECEIPT_MINT_PDA_SEED_V2,
            STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2,
        ] {
            assert!(
                domain.len() <= MAX_PDA_SEED_BYTES,
                "a PDA seed domain longer than 32 bytes can never derive an address"
            );
        }
    }

    #[test]
    fn seed_domains_are_the_exact_published_spellings() {
        assert_eq!(STRUCTURED_ROOT_PDA_SEED_V2, b"dclutch:structured-root:v2");
        assert_eq!(
            STRUCTURED_RECEIPT_MINT_PDA_SEED_V2,
            b"dclutch:struct-receipt-mint:v2"
        );
        assert_eq!(
            STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2,
            b"dclutch:struct-shard-custody:v2"
        );
    }

    #[test]
    fn root_seed_order_is_exact_and_the_address_join_is_closed() {
        let seeds = StructuredRootSeedsV2::new(id(1)).expect("seeds");
        assert_eq!(
            seeds.as_slices(),
            [STRUCTURED_ROOT_PDA_SEED_V2, id(1).as_slice()]
        );
        assert_eq!(seeds.authenticate_address(id(4), id(4)), Ok(id(4)));
        assert_eq!(
            seeds.authenticate_address(id(4), id(5)),
            Err(StructuredSeedErrorV2::AddressMismatch)
        );
        assert_eq!(
            seeds.authenticate_address(id(1), id(1)),
            Err(StructuredSeedErrorV2::AccountAlias)
        );
        assert_eq!(
            StructuredRootSeedsV2::new([0; 32]),
            Err(StructuredSeedErrorV2::ZeroIdentity)
        );
    }

    #[test]
    fn a_root_is_one_per_terms_and_two_terms_never_share_one() {
        let left = StructuredRootSeedsV2::new(id(1)).expect("left");
        let right = StructuredRootSeedsV2::new(id(2)).expect("right");
        assert_ne!(left.as_slices(), right.as_slices());
    }

    #[test]
    fn receipt_mint_seed_order_is_exact_and_nonaliasing() {
        let seeds = StructuredReceiptMintSeedsV2::new(id(1), id(2), id(3)).expect("seeds");
        assert_eq!(
            seeds.as_slices(),
            [
                STRUCTURED_RECEIPT_MINT_PDA_SEED_V2,
                id(1).as_slice(),
                id(2).as_slice(),
                id(3).as_slice(),
            ]
        );
        assert_eq!(seeds.authenticate_address(id(9), id(9)), Ok(id(9)));
        assert_eq!(
            seeds.authenticate_address(id(9), id(8)),
            Err(StructuredSeedErrorV2::AddressMismatch)
        );
        for alias in [id(1), id(2), id(3)] {
            assert_eq!(
                seeds.authenticate_address(alias, alias),
                Err(StructuredSeedErrorV2::AccountAlias)
            );
        }
        for hostile in [
            StructuredReceiptMintSeedsV2::new([0; 32], id(2), id(3)),
            StructuredReceiptMintSeedsV2::new(id(1), [0; 32], id(3)),
            StructuredReceiptMintSeedsV2::new(id(1), id(2), [0; 32]),
            StructuredReceiptMintSeedsV2::new(id(1), id(1), id(3)),
            StructuredReceiptMintSeedsV2::new(id(1), id(2), id(1)),
            StructuredReceiptMintSeedsV2::new(id(1), id(2), id(2)),
        ] {
            assert!(hostile.is_err());
        }
    }

    #[test]
    fn shard_custody_is_keyed_by_mint_so_two_coordinates_never_collide() {
        let left = StructuredShardCustodySeedsV2::new(id(1), id(7)).expect("left");
        let right = StructuredShardCustodySeedsV2::new(id(1), id(8)).expect("right");
        assert_ne!(left.as_slices(), right.as_slices());
        assert_eq!(
            left.as_slices(),
            [
                STRUCTURED_SHARD_CUSTODY_PDA_SEED_V2,
                id(1).as_slice(),
                id(7).as_slice(),
            ]
        );
        assert_eq!(left.authenticate_address(id(9), id(9)), Ok(id(9)));
        assert_eq!(
            left.authenticate_address(id(7), id(7)),
            Err(StructuredSeedErrorV2::AccountAlias)
        );
        assert_eq!(
            StructuredShardCustodySeedsV2::new(id(1), id(1)),
            Err(StructuredSeedErrorV2::AccountAlias)
        );
    }

    const WIDTH: u32 = 2;
    const TERMS_BYTES: usize =
        STRUCTURED_TERMS_HEADER_BYTES_V2 + 2 * STRUCTURED_TERMS_COEFFICIENT_BYTES_V2;
    const PREIMAGE_BYTES: usize =
        STRUCTURED_RECEIPT_MINT_PREIMAGE_DOMAIN_V2.len() + TERMS_BYTES - 32;

    #[test]
    fn the_declared_widths_agree_with_the_lean_emitted_layout() {
        assert_eq!(structured_terms_bytes_for_width_v2(WIDTH), Ok(TERMS_BYTES));
        assert_eq!(
            structured_receipt_mint_preimage_bytes_v2(WIDTH),
            Ok(PREIMAGE_BYTES)
        );
    }

    #[test]
    fn the_preimage_excises_exactly_the_persisted_mint_field() {
        let mut terms = [0_u8; TERMS_BYTES];
        for (index, byte) in terms.iter_mut().enumerate() {
            *byte = u8::try_from(index % 251).expect("byte");
        }
        let mut preimage = [0_u8; PREIMAGE_BYTES];
        structured_receipt_mint_preimage_v2(&terms, WIDTH, &mut preimage).expect("preimage");
        let domain = STRUCTURED_RECEIPT_MINT_PREIMAGE_DOMAIN_V2.len();
        assert_eq!(
            preimage.get(..domain),
            Some(STRUCTURED_RECEIPT_MINT_PREIMAGE_DOMAIN_V2)
        );
        assert_eq!(
            preimage.get(domain..domain + STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2),
            terms.get(..STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2)
        );
        assert_eq!(
            preimage.get(domain + STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2..),
            terms.get(STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2 + 32..)
        );
    }

    #[test]
    fn the_preimage_moves_with_a_coefficient_and_not_with_the_mint() {
        let mut canonical = [7_u8; TERMS_BYTES];
        let mut left = [0_u8; PREIMAGE_BYTES];
        structured_receipt_mint_preimage_v2(&canonical, WIDTH, &mut left).expect("left");

        // Moving the persisted Mint field alone must not move the preimage --
        // that is exactly the circularity the excision exists to break.
        canonical[STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2] = 99;
        let mut mint_moved = [0_u8; PREIMAGE_BYTES];
        structured_receipt_mint_preimage_v2(&canonical, WIDTH, &mut mint_moved).expect("mint");
        assert_eq!(left, mint_moved);

        // Moving one coefficient byte must move it, so two terms differing only
        // in a coefficient can never derive the same receipt Mint.
        canonical[STRUCTURED_TERMS_HEADER_BYTES_V2] = 99;
        let mut coefficient_moved = [0_u8; PREIMAGE_BYTES];
        structured_receipt_mint_preimage_v2(&canonical, WIDTH, &mut coefficient_moved)
            .expect("coefficient");
        assert_ne!(left, coefficient_moved);
    }

    #[test]
    fn a_mis_sized_terms_slice_output_buffer_or_declared_width_refuses() {
        let terms = [0_u8; TERMS_BYTES];
        let short_terms = [0_u8; TERMS_BYTES - 1];
        let mut output = [0_u8; PREIMAGE_BYTES];
        let mut short_output = [0_u8; PREIMAGE_BYTES - 1];
        assert_eq!(
            structured_receipt_mint_preimage_v2(&short_terms, WIDTH, &mut output),
            Err(StructuredSeedErrorV2::InvalidLength)
        );
        assert_eq!(
            structured_receipt_mint_preimage_v2(&terms, WIDTH, &mut short_output),
            Err(StructuredSeedErrorV2::InvalidLength)
        );
        // The same bytes read at a DIFFERENT declared width also refuse, so a
        // width substitution cannot silently reinterpret the coefficient tail.
        assert_eq!(
            structured_receipt_mint_preimage_v2(&terms, WIDTH + 1, &mut output),
            Err(StructuredSeedErrorV2::InvalidLength)
        );
    }
}
