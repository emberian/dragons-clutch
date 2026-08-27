//! Onchain-safe Structured V2 execution candidate for common Trading Hot.
//!
//! The candidate is opaque: it is prepared from independently authenticated
//! inputs, revalidates every amount against the immutable coefficients, and
//! exposes only borrowed effects plus commit-last root bytes.  It contains no
//! Claims child, because a Structured receipt's single backing edge points at
//! the claim-shard layer and never past it.

use dclutch_fractional_claim_kernel::FractionalExposureTermsV2;
use dclutch_structured_v2_kernel::{
    STRUCTURED_NO_COORDINATE_V2, STRUCTURED_ROOT_BYTES_V2, StructuredTermsV2,
};

use crate::{StructuredActionV2, StructuredRequestV2, StructuredRootV2};

/// Account-profile bound: one receipt effect plus one effect per backed coordinate.
///
/// This is a measured-profile bound derived from the Structured capacity profile
/// (`K <= 256`), not a mathematical limit.  Lifting it requires the paged
/// account/manifest profile, not a wider constant here.
pub const STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2: usize = 257;

/// Stable refusal from candidate preparation or postcondition validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredHotErrorV2 {
    /// Request, terms, shard terms, root, or immutable identity differed.
    IdentityMismatch,
    /// An account coordinate or key was absent, aliased, or noncanonical.
    AccountMismatch,
    /// Token effect count, order, kind, amount, or pre/post state differed.
    TokenMismatch,
    /// Root revision or exact candidate bytes differed.
    RootMismatch,
    /// Lifecycle-Rent closure coordinates were absent, extra, or noncanonical.
    RentMismatch,
    /// Checked integer arithmetic overflowed or underflowed.
    Arithmetic,
}

/// Result alias for the Structured V2 Hot contract.
pub type Result<T> = core::result::Result<T, StructuredHotErrorV2>;

/// Exact account identity and coordinate in the authenticated AccountProfile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredHotAccountRefV2 {
    coordinate: u16,
    key: [u8; 32],
}

impl StructuredHotAccountRefV2 {
    /// Construct one nonzero account identity at a concrete profile coordinate.
    pub fn new(coordinate: u16, key: [u8; 32]) -> Result<Self> {
        if is_zero(key) {
            return Err(StructuredHotErrorV2::AccountMismatch);
        }
        Ok(Self { coordinate, key })
    }
    /// AccountProfile coordinate.
    pub const fn coordinate(self) -> u16 {
        self.coordinate
    }
    /// Exact authenticated account identity.
    pub const fn key(self) -> [u8; 32] {
        self.key
    }
}

/// Exact Token-2022 effect selected by one Structured V2 action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredHotTokenKindV2 {
    /// Mint exact receipt atoms to the actor.
    MintReceipts,
    /// Permissioned burn of exact receipt atoms from the actor.
    BurnReceipts,
    /// Transfer the exact coefficient basket into Structured shard custody.
    LockShards,
    /// Transfer the exact coefficient basket out of Structured shard custody.
    ReleaseShards,
    /// Close one zero-balance shard custody account during retirement.
    CloseCustody,
    /// Close the zero-supply receipt Mint during retirement.
    CloseReceiptMint,
}

/// One fixed Token-2022 effect and its independently observed pre/post state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredHotTokenEffectV2 {
    /// Exact effect kind.
    pub kind: StructuredHotTokenKindV2,
    /// Representation coordinate, or the canonical absent sentinel for receipts.
    pub representation_coordinate: u32,
    /// Terms-selected Token program.
    pub token_program: StructuredHotAccountRefV2,
    /// Receipt Mint or shard-terms-selected shard Mint.
    pub mint: StructuredHotAccountRefV2,
    /// Source Token account when active.
    pub source: Option<StructuredHotAccountRefV2>,
    /// Destination Token account or RentCredit when active.
    pub destination: Option<StructuredHotAccountRefV2>,
    /// Exact signing authority: root, or the actor for a lock transfer.
    pub authority: StructuredHotAccountRefV2,
    /// Exact raw base units; zero only for the two closure kinds.
    pub amount: u64,
    /// Mint supply before the effect.
    pub pre_supply: u64,
    /// Mint supply after the effect.
    pub post_supply: u64,
    /// Source amount before the effect, zero when absent.
    pub pre_source: u64,
    /// Source amount after the effect, zero when absent.
    pub post_source: u64,
    /// Destination amount before the effect, zero when absent.
    pub pre_destination: u64,
    /// Destination amount after the effect, zero when absent.
    pub post_destination: u64,
}

/// Lifecycle-Rent close coordinates for zero-supply retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredHotRentCloseV2 {
    /// Selected Rent program.
    pub rent_program: StructuredHotAccountRefV2,
    /// Root-bound lifecycle RentCredit.
    pub rent_credit: StructuredHotAccountRefV2,
    /// First coordinate of the exact Rent close frame.
    pub route_base: u16,
    /// Core-authenticated producer-subtree postresource digest.
    pub post_resource_digest: [u8; 32],
}

/// All chain-derived inputs required to prepare one nonforgeable candidate.
#[derive(Clone, Copy, Debug)]
pub struct StructuredHotCandidateInputV2<'a> {
    /// Exact hostile-decoded request.
    pub request: StructuredRequestV2,
    /// Exact authenticated immutable Structured terms.
    pub terms: StructuredTermsV2<'a>,
    /// Exact authenticated immutable claim-shard terms.
    pub shard_terms: FractionalExposureTermsV2<'a>,
    /// Exact authenticated root bytes.
    pub root_bytes: &'a [u8],
    /// Root account identity and AccountProfile coordinate.
    pub root: StructuredHotAccountRefV2,
    /// Ordered Token effects; the count is action-derived.
    pub token_effects: &'a [StructuredHotTokenEffectV2],
    /// Lifecycle-Rent close only for `ZeroSupplyRetire`.
    pub rent_close: Option<StructuredHotRentCloseV2>,
}

/// Opaque onchain-safe candidate consumed by common Trading Hot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredHotCandidateV2<'a> {
    request: StructuredRequestV2,
    root: StructuredHotAccountRefV2,
    token_effects: &'a [StructuredHotTokenEffectV2],
    rent_close: Option<StructuredHotRentCloseV2>,
    pre_revision: u64,
    post_revision: Option<u64>,
    root_candidate: Option<[u8; STRUCTURED_ROOT_BYTES_V2]>,
}

impl<'a> StructuredHotCandidateV2<'a> {
    /// Prepare and fully validate one bounded candidate.
    pub fn prepare(input: StructuredHotCandidateInputV2<'a>) -> Result<Self> {
        let request = input
            .request
            .bind_terms(input.terms)
            .map_err(|_| StructuredHotErrorV2::IdentityMismatch)?;
        input
            .terms
            .bind_shard_terms(input.shard_terms)
            .map_err(|_| StructuredHotErrorV2::IdentityMismatch)?;
        let root =
            StructuredRootV2::decode(input.root_bytes).ok_or(StructuredHotErrorV2::RootMismatch)?;
        if root.input().terms != input.terms.terms_id()
            || root.input().market != input.terms.market()
            || root.input().revision != request.input().expected_revision
            || input.token_effects.len() > STRUCTURED_HOT_MAX_TOKEN_EFFECTS_V2
        {
            return Err(StructuredHotErrorV2::IdentityMismatch);
        }
        let retiring = request.action() == StructuredActionV2::ZeroSupplyRetire;
        if retiring != input.rent_close.is_some() {
            return Err(StructuredHotErrorV2::RentMismatch);
        }
        if let Some(close) = input.rent_close
            && is_zero(close.post_resource_digest)
        {
            return Err(StructuredHotErrorV2::RentMismatch);
        }
        validate_token_effects(input, request)?;
        let (post_revision, root_candidate) = if retiring {
            (None, None)
        } else {
            let advanced = root.advanced().ok_or(StructuredHotErrorV2::Arithmetic)?;
            (Some(advanced.input().revision), Some(advanced.to_bytes()))
        };
        Ok(Self {
            request,
            root: input.root,
            token_effects: input.token_effects,
            rent_close: input.rent_close,
            pre_revision: root.input().revision,
            post_revision,
            root_candidate,
        })
    }

    /// Exact accepted action.
    pub const fn action(self) -> StructuredActionV2 {
        self.request.action()
    }
    /// Exact accepted request.
    pub const fn request(self) -> StructuredRequestV2 {
        self.request
    }
    /// Exact root account reference.
    pub const fn root(self) -> StructuredHotAccountRefV2 {
        self.root
    }
    /// Ordered bounded Token effects.
    pub const fn token_effects(self) -> &'a [StructuredHotTokenEffectV2] {
        self.token_effects
    }
    /// Lifecycle-Rent close coordinates, present only for retirement.
    pub const fn rent_close(self) -> Option<StructuredHotRentCloseV2> {
        self.rent_close
    }
    /// Pre-revision authenticated from root bytes.
    pub const fn pre_revision(self) -> u64 {
        self.pre_revision
    }
    /// Post-revision, absent only when retirement closes the root.
    pub const fn post_revision(self) -> Option<u64> {
        self.post_revision
    }
    /// Copy exact root candidate bytes for commit-last execution.
    pub const fn root_candidate_bytes(self) -> Option<[u8; STRUCTURED_ROOT_BYTES_V2]> {
        self.root_candidate
    }

    /// Recheck exact Token-owned post observations after every Token CPI.
    pub fn validate_token_poststate(self, observed: &[StructuredHotTokenPostV2]) -> Result<()> {
        if observed.len() != self.token_effects.len() {
            return Err(StructuredHotErrorV2::TokenMismatch);
        }
        for (effect, actual) in self.token_effects.iter().zip(observed) {
            if actual.representation_coordinate != effect.representation_coordinate
                || actual.mint != effect.mint.key()
                || actual.supply != effect.post_supply
                || actual.source_amount != effect.post_source
                || actual.destination_amount != effect.post_destination
            {
                return Err(StructuredHotErrorV2::TokenMismatch);
            }
        }
        Ok(())
    }

    /// Require the exact root candidate or exact terminal close.
    pub fn validate_root_poststate(self, actual: Option<&[u8]>) -> Result<()> {
        match (self.root_candidate, actual) {
            (Some(expected), Some(actual)) if actual == expected => Ok(()),
            (None, None) => Ok(()),
            _ => Err(StructuredHotErrorV2::RootMismatch),
        }
    }
}

/// Token-owned facts re-read immediately after one effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredHotTokenPostV2 {
    /// Representation coordinate, or the canonical absent sentinel for receipts.
    pub representation_coordinate: u32,
    /// Exact selected Mint.
    pub mint: [u8; 32],
    /// Exact post supply.
    pub supply: u64,
    /// Exact post source amount, zero when absent.
    pub source_amount: u64,
    /// Exact post destination amount, zero when absent.
    pub destination_amount: u64,
}

fn validate_token_effects(
    input: StructuredHotCandidateInputV2<'_>,
    request: StructuredRequestV2,
) -> Result<()> {
    let backed = backed_coordinate_count(input.terms)?;
    let expected_count = backed
        .checked_add(1)
        .ok_or(StructuredHotErrorV2::Arithmetic)?;
    if input.token_effects.len() != expected_count {
        return Err(StructuredHotErrorV2::TokenMismatch);
    }
    let retiring = request.action() == StructuredActionV2::ZeroSupplyRetire;
    let mut coordinate = 0_u32;
    let mut prior_mint_coordinate: Option<u16> = None;
    for (index, effect) in input.token_effects.iter().copied().enumerate() {
        validate_account_plan(input.root, effect)?;
        if effect.token_program.key() != input.terms.token_program() {
            return Err(StructuredHotErrorV2::TokenMismatch);
        }
        // The receipt effect is first for every supply-changing action and last
        // for retirement, so the closure sweep runs before the Mint closes.
        let receipt_slot = if retiring { backed } else { 0 };
        if index == receipt_slot {
            validate_receipt_effect(input, request, effect)?;
            continue;
        }
        // The K-width shard sweep is canonically ordered: strictly ascending
        // Mint coordinates, so no coordinate can be duplicated or reordered.
        if let Some(prior) = prior_mint_coordinate
            && effect.mint.coordinate() <= prior
        {
            return Err(StructuredHotErrorV2::AccountMismatch);
        }
        prior_mint_coordinate = Some(effect.mint.coordinate());
        coordinate = next_backed_coordinate(input.terms, coordinate)?;
        validate_shard_effect(input, request, effect, coordinate)?;
        coordinate = coordinate
            .checked_add(1)
            .ok_or(StructuredHotErrorV2::Arithmetic)?;
    }
    Ok(())
}

fn validate_receipt_effect(
    input: StructuredHotCandidateInputV2<'_>,
    request: StructuredRequestV2,
    effect: StructuredHotTokenEffectV2,
) -> Result<()> {
    let fields = request.input();
    if effect.representation_coordinate != STRUCTURED_NO_COORDINATE_V2
        || effect.mint.key() != input.terms.receipt_mint()
        || effect.authority != input.root
    {
        return Err(StructuredHotErrorV2::TokenMismatch);
    }
    match request.action() {
        StructuredActionV2::Issue => {
            if effect.kind != StructuredHotTokenKindV2::MintReceipts
                || effect.amount != fields.quantity
                || effect.source.is_some()
                || effect.destination.map(StructuredHotAccountRefV2::key)
                    != Some(fields.receipt_destination)
                || effect.post_supply != checked_add(effect.pre_supply, effect.amount)?
                || effect.pre_source != 0
                || effect.post_source != 0
                || effect.post_destination != checked_add(effect.pre_destination, effect.amount)?
            {
                return Err(StructuredHotErrorV2::TokenMismatch);
            }
        }
        StructuredActionV2::Unwrap | StructuredActionV2::TerminalRedeem => {
            if effect.kind != StructuredHotTokenKindV2::BurnReceipts
                || effect.amount != fields.quantity
                || effect.source.map(StructuredHotAccountRefV2::key) != Some(fields.receipt_source)
                || effect.destination.is_some()
                || effect.post_supply != checked_sub(effect.pre_supply, effect.amount)?
                || effect.post_source != checked_sub(effect.pre_source, effect.amount)?
                || effect.pre_destination != 0
                || effect.post_destination != 0
            {
                return Err(StructuredHotErrorV2::TokenMismatch);
            }
        }
        StructuredActionV2::ZeroSupplyRetire => {
            let rent_credit = input
                .rent_close
                .map(|close| close.rent_credit)
                .ok_or(StructuredHotErrorV2::RentMismatch)?;
            if effect.kind != StructuredHotTokenKindV2::CloseReceiptMint
                || effect.amount != 0
                || effect.source.is_some()
                || effect.destination != Some(rent_credit)
                || effect.pre_supply != 0
                || effect.post_supply != 0
                || [
                    effect.pre_source,
                    effect.post_source,
                    effect.pre_destination,
                    effect.post_destination,
                ]
                .into_iter()
                .any(|value| value != 0)
            {
                return Err(StructuredHotErrorV2::TokenMismatch);
            }
        }
    }
    Ok(())
}

fn validate_shard_effect(
    input: StructuredHotCandidateInputV2<'_>,
    request: StructuredRequestV2,
    effect: StructuredHotTokenEffectV2,
    coordinate: u32,
) -> Result<()> {
    let fields = request.input();
    let expected_mint = input
        .shard_terms
        .shard_mint(coordinate)
        .map_err(|_| StructuredHotErrorV2::IdentityMismatch)?;
    if effect.representation_coordinate != coordinate || effect.mint.key() != expected_mint {
        return Err(StructuredHotErrorV2::TokenMismatch);
    }
    let coefficient = input
        .terms
        .coefficient(coordinate)
        .map_err(|_| StructuredHotErrorV2::IdentityMismatch)?;
    let basket = fields
        .quantity
        .checked_mul(coefficient)
        .ok_or(StructuredHotErrorV2::Arithmetic)?;
    match request.action() {
        StructuredActionV2::Issue => {
            if effect.kind != StructuredHotTokenKindV2::LockShards
                || effect.amount != basket
                || effect.authority.key() != fields.owner
                || effect.source.is_none()
                || effect.destination.is_none()
                || effect.post_supply != effect.pre_supply
                || effect.post_source != checked_sub(effect.pre_source, basket)?
                || effect.post_destination != checked_add(effect.pre_destination, basket)?
            {
                return Err(StructuredHotErrorV2::TokenMismatch);
            }
        }
        StructuredActionV2::Unwrap | StructuredActionV2::TerminalRedeem => {
            if effect.kind != StructuredHotTokenKindV2::ReleaseShards
                || effect.amount != basket
                || effect.authority != input.root
                || effect.source.is_none()
                || effect.destination.is_none()
                || effect.post_supply != effect.pre_supply
                || effect.post_source != checked_sub(effect.pre_source, basket)?
                || effect.post_destination != checked_add(effect.pre_destination, basket)?
            {
                return Err(StructuredHotErrorV2::TokenMismatch);
            }
        }
        StructuredActionV2::ZeroSupplyRetire => {
            let rent_credit = input
                .rent_close
                .map(|close| close.rent_credit)
                .ok_or(StructuredHotErrorV2::RentMismatch)?;
            if effect.kind != StructuredHotTokenKindV2::CloseCustody
                || effect.amount != 0
                || effect.authority != input.root
                || effect.source.is_none()
                || effect.destination != Some(rent_credit)
                || effect.post_supply != effect.pre_supply
                || [
                    effect.pre_source,
                    effect.post_source,
                    effect.pre_destination,
                    effect.post_destination,
                ]
                .into_iter()
                .any(|value| value != 0)
            {
                return Err(StructuredHotErrorV2::TokenMismatch);
            }
        }
    }
    Ok(())
}

fn validate_account_plan(
    root: StructuredHotAccountRefV2,
    effect: StructuredHotTokenEffectV2,
) -> Result<()> {
    let accounts = [
        Some(effect.token_program),
        Some(effect.mint),
        effect.source,
        effect.destination,
        Some(effect.authority),
    ];
    for (left_index, left) in accounts.iter().enumerate() {
        let Some(left) = left else { continue };
        for right in accounts.iter().skip(left_index + 1).flatten() {
            if left.coordinate() == right.coordinate() || left.key() == right.key() {
                return Err(StructuredHotErrorV2::AccountMismatch);
            }
        }
    }
    if effect.authority.key() == root.key() && effect.authority != root {
        return Err(StructuredHotErrorV2::AccountMismatch);
    }
    Ok(())
}

fn backed_coordinate_count(terms: StructuredTermsV2<'_>) -> Result<usize> {
    let mut count = 0_usize;
    let mut coordinate = 0_u32;
    while coordinate < terms.representation_width() {
        if terms
            .coefficient(coordinate)
            .map_err(|_| StructuredHotErrorV2::IdentityMismatch)?
            != 0
        {
            count = count
                .checked_add(1)
                .ok_or(StructuredHotErrorV2::Arithmetic)?;
        }
        coordinate = coordinate
            .checked_add(1)
            .ok_or(StructuredHotErrorV2::Arithmetic)?;
    }
    Ok(count)
}

fn next_backed_coordinate(terms: StructuredTermsV2<'_>, from: u32) -> Result<u32> {
    let mut coordinate = from;
    while coordinate < terms.representation_width() {
        if terms
            .coefficient(coordinate)
            .map_err(|_| StructuredHotErrorV2::IdentityMismatch)?
            != 0
        {
            return Ok(coordinate);
        }
        coordinate = coordinate
            .checked_add(1)
            .ok_or(StructuredHotErrorV2::Arithmetic)?;
    }
    Err(StructuredHotErrorV2::TokenMismatch)
}

fn checked_add(value: u64, amount: u64) -> Result<u64> {
    value
        .checked_add(amount)
        .ok_or(StructuredHotErrorV2::Arithmetic)
}

fn checked_sub(value: u64, amount: u64) -> Result<u64> {
    value
        .checked_sub(amount)
        .ok_or(StructuredHotErrorV2::TokenMismatch)
}

fn is_zero(value: [u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}
